// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Juan Manuel Peralta Chacon (GitHub: juanmanuel767)
//! El relé en sí (pieza de servidor, licencia AGPL-3.0): buzones, empalme de conexiones, cartas,
//! avisos, límites y velocidad. Lo de cliente (lo que usan las apps) está en `relay.rs`, con MIT.

use mobilemesh_core::{
    Result,
    identity::{from_hex, from_hex_any, to_hex},
    relay::{CONTEXT, MAX_LETTER, WAIT, id_of, read_line, read_line_max},
};
use ed25519_dalek::{Signature, VerifyingKey};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Ipv6Addr, Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
/// La conexión de control manda «PING» cada 25 s; si calla más que esto, se cierra.
const SILENCE: Duration = Duration::from_secs(90);
/// Una conexión empalmada sin tráfico durante este tiempo se cierra.
const IDLE: Duration = Duration::from_secs(300);
// ponytail: límites fijos; hacerlos configurables cuando haya relés públicos con mucha gente.
const MAX_CONNECTIONS: usize = 1024;
// Altos a propósito: con datos móviles cientos de teléfonos salen por la misma IP de la operadora.
// Pero nunca más de un cuarto del total: una sola IP no puede dejar fuera a todos los demás.
const MAX_PER_IP: usize = 1000;
const MAX_BOXES_PER_IP: usize = 1000;
const MAX_PENDING: usize = 256;
const MAX_PENDING_PER_BOX: usize = 4;
const MAX_LETTERS_PER_BOX: usize = 200;
const MAX_LETTER_BYTES: usize = 64 * 1024 * 1024;
/// Lo que ocupa en memoria cada carta aparte de su texto (ficha, hora, buzón): así un aluvión de
/// cartas de 1 byte también choca con `MAX_LETTER_BYTES`.
const LETTER_OVERHEAD: usize = 256;
/// Las cartas caducadas se buscan como mucho una vez por este tiempo (recorrerlas todas cuesta).
const PURGE_EVERY: Duration = Duration::from_secs(60);
/// Tiempo total para mandar una orden (por lentos que lleguen los bytes, sin goteo eterno).
const LINE_DEADLINE: Duration = Duration::from_secs(30);
/// Pila de cada hilo: la de serie (2 MB) sobra para leer y copiar bytes.
const STACK: usize = 256 * 1024;

/// Un hilo con la pila pequeña. Si el sistema no da más hilos, devuelve el error: se rechaza esa
/// conexión en vez de caerse el relé entero (`thread::spawn` aborta en ese caso).
fn hilo<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> std::io::Result<thread::JoinHandle<T>> {
    thread::Builder::new().stack_size(STACK).spawn(f)
}
const LETTER_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// Como mucho un aviso por buzón cada este tiempo (varias cartas seguidas = un aviso).
const NOTIFY_EVERY: Duration = Duration::from_secs(20);

#[derive(Default)]
struct State {
    /// buzón → (conexión de control de su dueño, IP del dueño)
    boxes: HashMap<String, (TcpStream, IpAddr)>,
    /// ficha → (buzón, conexión de quien marcó) esperando a que el dueño acepte
    pending: HashMap<String, (String, TcpStream)>,
    /// conexiones abiertas por IP
    conns: HashMap<IpAddr, usize>,
    /// buzón → cartas (ficha, carta, cuándo llegó)
    letters: HashMap<String, Vec<(String, String, std::time::Instant)>>,
    letter_bytes: usize,
    last_purge: Option<Instant>,
    /// buzón → (ficha de avisos, último aviso)
    push: HashMap<String, (String, Option<std::time::Instant>)>,
}

impl State {
    fn purge_old_letters(&mut self) {
        if self.last_purge.is_some_and(|t| t.elapsed() < PURGE_EVERY) {
            return;
        }
        self.last_purge = Some(Instant::now());
        let mut freed = 0;
        for list in self.letters.values_mut() {
            list.retain(|(_, l, at)| {
                let keep = at.elapsed() < LETTER_TTL;
                if !keep {
                    freed += l.len() + LETTER_OVERHEAD;
                }
                keep
            });
        }
        self.letters.retain(|_, l| !l.is_empty());
        self.letter_bytes -= freed;
    }

    /// Guarda una carta si caben (por buzón y en total). Devuelve si la guardó.
    fn keep_letter(&mut self, id: &str, letter: &str) -> Result<bool> {
        self.purge_old_letters();
        let full = self.letters.get(id).is_some_and(|l| l.len() >= MAX_LETTERS_PER_BOX);
        let cost = letter.len() + LETTER_OVERHEAD;
        if full || self.letter_bytes + cost > MAX_LETTER_BYTES {
            return Ok(false);
        }
        let mut token = [0u8; 16];
        getrandom::fill(&mut token).map_err(|e| e.to_string())?;
        self.letter_bytes += cost;
        self.letters.entry(id.to_string()).or_default().push((to_hex(&token), letter.to_string(), Instant::now()));
        Ok(true)
    }
}

/// La IP con la que se cuentan los límites: en IPv6 cada casa recibe un /64 entero, así que se
/// cuenta por /64 (si no, cambiar de dirección dentro de él saltaría el límite).
pub(crate) fn ip_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.into(),
            None => {
                let s = v6.segments();
                Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0).into()
            }
        },
        v4 => v4,
    }
}

/// Lector con plazo total: cada lectura espera solo lo que queda hasta `1`, así mandar un byte
/// cada pocos segundos no mantiene ocupada la conexión para siempre.
struct Deadline<'a>(&'a TcpStream, Instant);

impl Deadline<'_> {
    fn of(s: &TcpStream) -> Deadline<'_> {
        Deadline(s, Instant::now() + LINE_DEADLINE)
    }
}

impl Read for Deadline<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let left = self.1.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        self.0.set_read_timeout(Some(left))?;
        (&mut &*self.0).read(buf)
    }
}

/// Un relé en marcha. `forwarded` cuenta los bytes reenviados (para medir lo que cuesta mantenerlo).
#[derive(Clone, Default)]
pub struct Relay {
    state: Arc<Mutex<State>>,
    pub forwarded: Arc<AtomicU64>,
    /// Llamadas empalmadas en este momento.
    calls: Arc<AtomicU64>,
    /// Programa que se ejecuta con la ficha de avisos al llegar una carta (opcional).
    pub notifier: Option<String>,
    /// Velocidad máxima, sumando todas las conexiones empalmadas (bytes por segundo), para que el
    /// relé no deje sin internet la casa donde está. `None`: sin límite.
    pub limite: Option<u64>,
    ventana: Arc<Mutex<Option<(std::time::Instant, u64)>>>,
    /// Archivo con el directorio firmado que se entrega a quien lo pida (opcional).
    pub directorio: Option<String>,
    /// Conexiones a la vez (por defecto `MAX_CONNECTIONS`). Más necesita más archivos abiertos
    /// (`ulimit -n`): dos por cada llamada empalmada.
    pub max: Option<usize>,
}

impl Relay {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Atiende conexiones para siempre.
    pub fn serve(&self, listener: TcpListener) {
        for stream in listener.incoming().flatten() {
            let _ = stream.set_nodelay(true); // sin esperas de Nagle en las órdenes cortas
            let Ok(ip) = stream.peer_addr().map(|a| a.ip()) else { continue };
            let key = ip_key(ip);
            {
                let mut st = self.lock();
                let total: usize = st.conns.values().sum();
                let mine = st.conns.get(&key).copied().unwrap_or(0);
                let max = self.max.unwrap_or(MAX_CONNECTIONS);
                // Las locales son del puente web, que ya limita por la IP real de cada navegador.
                if total >= max || (mine >= MAX_PER_IP.min(max / 4).max(1) && !ip.is_loopback()) {
                    continue; // se cierra sin contestar
                }
                st.conns.insert(key, mine + 1);
            }
            let me = self.clone();
            if hilo(move || {
                let _ = me.handle(stream, ip);
                me.release(key);
            })
            .is_err()
            {
                self.release(key); // sin hilo: la conexión se cierra y deja de contar
            }
        }
    }

    /// Números del relé en JSON (solo cuentas, nada de quién: el relé no guarda registros).
    pub fn estado(&self) -> String {
        let st = self.lock();
        format!(
            r#"{{"usuarios":{},"conexiones":{},"ips":{},"llamadas":{},"esperando":{},"cartas":{},"cartas_bytes":{},"avisos":{},"reenviado":{},"max":{}}}"#,
            st.boxes.len(),
            st.conns.values().sum::<usize>(),
            st.conns.len(),
            self.calls.load(Ordering::Relaxed),
            st.pending.len(),
            st.letters.values().map(Vec::len).sum::<usize>(),
            st.letter_bytes,
            st.push.len(),
            self.forwarded.load(Ordering::Relaxed),
            self.max.unwrap_or(MAX_CONNECTIONS),
        )
    }

    fn release(&self, key: IpAddr) {
        let mut st = self.lock();
        if let Some(n) = st.conns.get_mut(&key) {
            *n -= 1;
            if *n == 0 {
                st.conns.remove(&key);
            }
        }
    }

    fn handle(&self, mut s: TcpStream, ip: IpAddr) -> Result<()> {
        s.set_read_timeout(Some(WAIT))?;
        s.set_write_timeout(Some(WAIT))?;
        let line = read_line_max(&mut Deadline::of(&s), MAX_LETTER + 200)?;
        s.set_read_timeout(Some(WAIT))?;
        let (cmd, arg) = line.split_once(' ').unwrap_or((&line, ""));
        match cmd {
            "REGISTRAR" => self.register(s, ip, arg),
            "DEPOSITAR" => self.deposit_letter(s, arg),
            "RECOGER" => self.hand_over(s, arg),
            "AVISO" => self.set_push(s, arg),
            "MARCAR" => self.dial_box(s, arg),
            // Como STUN: «¿con qué IP me ves?» (la pública de mi router o de mi operadora).
            "QUIEN" => {
                writeln!(s, "{ip}")?;
                Ok(())
            }
            "DIRECTORIO" => {
                let d = self.directorio.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
                let line = d.as_deref().map(str::trim).filter(|d| !d.is_empty() && d.len() < 8192).unwrap_or("NO");
                writeln!(s, "{line}")?;
                Ok(())
            }
            "ACEPTAR" => {
                let caller = self.lock().pending.remove(arg);
                let Some((_, mut caller)) = caller else { return Ok(()) };
                caller.set_read_timeout(None)?;
                s.set_read_timeout(None)?;
                writeln!(caller, "OK")?;
                self.splice(caller, s);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Desafío: el dueño demuestra tener la clave del buzón sin enviarla. Devuelve el buzón.
    fn prove(&self, s: &mut TcpStream, public: &str) -> Result<String> {
        let public: [u8; 32] = from_hex(public)?.try_into().map_err(|_| "clave de buzón no válida")?;
        let key = VerifyingKey::from_bytes(&public).map_err(|_| "clave de buzón no válida")?;
        let mut nonce = [0u8; 32];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        writeln!(s, "RETO {}", to_hex(&nonce))?;
        let answer = read_line(&mut Deadline::of(s))?;
        let sig: [u8; 64] = from_hex_any(answer.strip_prefix("FIRMA ").ok_or("se esperaba la firma")?)?
            .try_into()
            .map_err(|_| "firma no válida")?;
        key.verify_strict(&[CONTEXT, &nonce].concat(), &Signature::from_bytes(&sig)).map_err(|_| "firma no válida")?;
        Ok(id_of(&public))
    }

    /// Guarda una carta para un buzón (aunque su dueño esté desconectado).
    fn deposit_letter(&self, mut s: TcpStream, arg: &str) -> Result<()> {
        let (id, letter) = arg.split_once(' ').ok_or("carta incompleta")?;
        let ok = id.len() == 32
            && !letter.is_empty()
            && letter.len() <= MAX_LETTER
            && letter.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
            && {
                let mut st = self.lock();
                if !st.keep_letter(id, letter)? {
                    false
                } else {
                    if let (Some(cmd), Some((push, last))) = (&self.notifier, st.push.get_mut(id))
                        && last.is_none_or(|t| t.elapsed() > NOTIFY_EVERY)
                    {
                        *last = Some(std::time::Instant::now());
                        // La ficha va como argumento, nunca dentro de una orden de la terminal.
                        let _ = std::process::Command::new(cmd).arg(push.as_str()).spawn().map(|mut c| {
                            thread::spawn(move || c.wait());
                        });
                    }
                    true
                }
            };
        writeln!(s, "{}", if ok { "OK" } else { "NO" })?;
        Ok(())
    }

    /// El dueño de un buzón guarda (o quita con «-») su ficha de avisos.
    fn set_push(&self, mut s: TcpStream, public: &str) -> Result<()> {
        let id = self.prove(&mut s, public)?;
        let token = read_line_max(&mut Deadline::of(&s), 4200)?;
        let valid = token.len() <= 4096 && token.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b));
        if !valid || token.is_empty() {
            return Err("ficha de avisos no válida".into());
        }
        let mut st = self.lock();
        if token == "-" {
            st.push.remove(&id);
        } else if st.push.len() < 100_000 || st.push.contains_key(&id) {
            st.push.insert(id, (token, None));
        }
        drop(st);
        writeln!(s, "OK")?;
        Ok(())
    }

    /// Entrega las cartas a su dueño y borra las que confirma.
    fn hand_over(&self, mut s: TcpStream, public: &str) -> Result<()> {
        let id = self.prove(&mut s, public)?;
        let letters: Vec<(String, String)> = {
            let mut st = self.lock();
            st.purge_old_letters();
            st.letters.get(&id).map(|l| l.iter().map(|(t, c, _)| (t.clone(), c.clone())).collect()).unwrap_or_default()
        };
        for (token, letter) in &letters {
            writeln!(s, "CARTA {token} {letter}")?;
        }
        writeln!(s, "FIN")?;
        while let Ok(line) = read_line(&mut Deadline::of(&s)) {
            let Some(token) = line.strip_prefix("RECIBIDA ") else { break };
            let mut st = self.lock();
            if let Some(list) = st.letters.get_mut(&id)
                && let Some(i) = list.iter().position(|(t, _, _)| t == token)
            {
                let (_, letter, _) = list.remove(i);
                st.letter_bytes -= letter.len() + LETTER_OVERHEAD;
            }
        }
        Ok(())
    }

    /// Registro del buzón para recibir llamadas.
    fn register(&self, mut s: TcpStream, ip: IpAddr, public: &str) -> Result<()> {
        let id = self.prove(&mut s, public)?;
        let ip = ip_key(ip);
        {
            let mut st = self.lock();
            if !ip.is_loopback() && st.boxes.values().filter(|(_, owner_ip)| *owner_ip == ip).count() >= MAX_BOXES_PER_IP {
                return Err("demasiados buzones desde esa dirección".into());
            }
            writeln!(s, "OK {id}")?;
            st.boxes.insert(id.clone(), (s.try_clone()?, ip));
        }
        // Se queda leyendo los PING; si calla demasiado o se cae, se libera el buzón.
        s.set_read_timeout(Some(SILENCE))?;
        let mut buf = [0u8; 64];
        while matches!(s.read(&mut buf), Ok(n) if n > 0) {}
        let mut st = self.lock();
        if st.boxes.get(&id).is_some_and(|(c, _)| c.peer_addr().ok() == s.peer_addr().ok()) {
            st.boxes.remove(&id);
        }
        Ok(())
    }

    fn dial_box(&self, mut s: TcpStream, id: &str) -> Result<()> {
        let mut token = [0u8; 16];
        getrandom::fill(&mut token).map_err(|e| e.to_string())?;
        let token = to_hex(&token);
        // Se copia la conexión del dueño y se escribe FUERA del candado: un dueño que no lee no
        // puede bloquear el relé entero (auditoría, Fase 13).
        let owner = {
            let mut st = self.lock();
            let waiting = st.pending.values().filter(|(b, _)| b == id).count();
            match st.boxes.get(id) {
                Some((owner, _)) if st.pending.len() < MAX_PENDING && waiting < MAX_PENDING_PER_BOX => {
                    let owner = owner.try_clone()?;
                    st.pending.insert(token.clone(), (id.to_string(), s.try_clone()?));
                    Some(owner)
                }
                _ => None,
            }
        };
        let notified = owner.is_some_and(|mut o| o.set_write_timeout(Some(WAIT)).is_ok() && writeln!(o, "LLAMADA {token}").is_ok());
        if !notified {
            self.lock().pending.remove(&token);
            writeln!(s, "NO")?;
            return Ok(());
        }
        // Si el dueño no acepta a tiempo, se cancela.
        thread::sleep(WAIT);
        if let Some((_, mut s)) = self.lock().pending.remove(&token) {
            let _ = writeln!(s, "NO");
        }
        Ok(())
    }

    /// Copia en los dos sentidos hasta que uno cierre o pase IDLE sin tráfico. No mira lo que pasa
    /// (va cifrado). Espera a que termine: así la conexión cuenta en los límites mientras dura.
    fn splice(&self, a: TcpStream, b: TcpStream) {
        for s in [&a, &b] {
            let _ = s.set_read_timeout(Some(IDLE));
        }
        let (limite, ventana) = (self.limite, self.ventana.clone());
        let pipe = |mut from: TcpStream, mut to: TcpStream, count: Arc<AtomicU64>| {
            let ventana = ventana.clone();
            move || {
                let mut buf = [0u8; 16 * 1024];
                while let Ok(n @ 1..) = from.read(&mut buf) {
                    if let Some(l) = limite {
                        frenar(&ventana, l, n as u64);
                    }
                    if to.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    count.fetch_add(n as u64, Ordering::Relaxed);
                }
                let _ = to.shutdown(Shutdown::Both);
                let _ = from.shutdown(Shutdown::Both);
            }
        };
        // Un sentido en un hilo nuevo y el otro en este: un hilo menos por llamada.
        if let (Ok(a2), Ok(b2)) = (a.try_clone(), b.try_clone())
            && let Ok(x) = hilo(pipe(a, b2, self.forwarded.clone()))
        {
            self.calls.fetch_add(1, Ordering::Relaxed);
            pipe(b, a2, self.forwarded.clone())();
            let _ = x.join();
            self.calls.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// Reparte `limite` bytes por segundo entre todas las conexiones: si ya se gastó lo de este
/// segundo, espera al siguiente. Un trozo siempre pasa (aunque sea mayor que el límite).
fn frenar(ventana: &Mutex<Option<(std::time::Instant, u64)>>, limite: u64, n: u64) {
    loop {
        let espera = {
            let mut v = ventana.lock().unwrap_or_else(|e| e.into_inner());
            let (inicio, usados) = v.get_or_insert((std::time::Instant::now(), 0));
            if inicio.elapsed() >= Duration::from_secs(1) {
                *inicio = std::time::Instant::now();
                *usados = 0;
            }
            if *usados == 0 || *usados + n <= limite {
                *usados += n;
                return;
            }
            Duration::from_secs(1).saturating_sub(inicio.elapsed())
        };
        thread::sleep(espera);
    }
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn el_limite_de_velocidad_frena_a_todos_juntos() {
        let ventana = std::sync::Mutex::new(None);
        let t = std::time::Instant::now();
        // 64 KB/s y 4 trozos de 32 KB → al menos 1 s de espera.
        for _ in 0..4 {
            super::frenar(&ventana, 64 * 1024, 32 * 1024);
        }
        assert!(t.elapsed() >= std::time::Duration::from_millis(900), "{:?}", t.elapsed());
        assert!(t.elapsed() < std::time::Duration::from_secs(3));
    }

    #[test]
    fn un_aluvion_de_cartas_pequenas_choca_con_el_tope() {
        let mut st = super::State::default();
        let mut n = 0;
        while st.keep_letter(&format!("{n:032}"), "A").unwrap() {
            n += 1;
        }
        // Cada carta cuenta su texto y lo que ocupa aparte: el tope llega mucho antes de agotar la memoria.
        assert_eq!(n, super::MAX_LETTER_BYTES / (1 + super::LETTER_OVERHEAD));
    }

    #[test]
    fn el_estado_cuenta_sin_decir_quien() {
        let r = super::Relay::default();
        r.lock().keep_letter(&"a".repeat(32), "carta").unwrap();
        let e = r.estado();
        assert!(e.starts_with('{') && e.ends_with('}'), "{e}");
        assert!(e.contains(r#""cartas":1"#) && e.contains(r#""usuarios":0"#), "{e}");
    }

    #[test]
    fn ipv6_se_cuenta_por_casa() {
        use std::net::IpAddr;
        let a: IpAddr = "2001:db8:1:2::1".parse().unwrap();
        let b: IpAddr = "2001:db8:1:2:ffff::9".parse().unwrap();
        let otra: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert_eq!(super::ip_key(a), super::ip_key(b));
        assert_ne!(super::ip_key(a), super::ip_key(otra));
        assert_eq!(super::ip_key("::ffff:1.2.3.4".parse().unwrap()), "1.2.3.4".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn el_goteo_no_mantiene_la_conexion() {
        use std::{io::Write, net::{TcpListener, TcpStream}, time::{Duration, Instant}};
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut c = TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let (s, _) = l.accept().unwrap();
        // Un byte cada 50 ms (cada lectura llega a tiempo), pero el plazo total es de 300 ms.
        std::thread::spawn(move || while c.write_all(b"x").is_ok() { std::thread::sleep(Duration::from_millis(50)) });
        let t = Instant::now();
        let r = mobilemesh_core::relay::read_line_max(&mut super::Deadline(&s, Instant::now() + Duration::from_millis(300)), 1000);
        assert!(r.is_err());
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
    }
}
