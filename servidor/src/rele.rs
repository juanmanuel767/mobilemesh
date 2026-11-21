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
        atomic::{AtomicU64, AtomicUsize, Ordering},
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
/// Lo que un solo origen (IP, o /64 en IPv6) puede tener guardado a la vez: un octavo del cupo
/// global. DEPOSITAR no autentica y el id del buzón lo elige quien deposita; sin cuota por origen,
/// un solo origen con unos pocos ids falsos llenaría `MAX_LETTER_BYTES` y dejaría sin buzón a todo
/// el mundo durante `LETTER_TTL` (7 días). Con esta cuota hacen falta 8 orígenes distintos.
/// Limitación: las conexiones que entran por el puente WebSocket llegan al relé como 127.0.0.1,
/// así que todos los clientes web comparten el mismo cubo de cuota y uno abusivo puede agotar la
/// parte de los clientes web. Arreglarlo exige que el puente pase la IP real al relé (cambio aparte).
const MAX_LETTER_BYTES_POR_IP: usize = MAX_LETTER_BYTES / 8;
/// Lo que ocupa en memoria cada carta aparte de su texto (ficha, hora, buzón): así un aluvión de
/// cartas de 1 byte también choca con `MAX_LETTER_BYTES`.
const LETTER_OVERHEAD: usize = 256;
/// Las cartas caducadas se buscan como mucho una vez por este tiempo (recorrerlas todas cuesta).
const PURGE_EVERY: Duration = Duration::from_secs(60);
/// Tiempo total para mandar una orden (por lentos que lleguen los bytes, sin goteo eterno).
const LINE_DEADLINE: Duration = Duration::from_secs(30);
/// Pila de cada hilo: la de serie (2 MB) sobra para leer y copiar bytes.
const STACK: usize = 256 * 1024;

/// Supernodos (móviles de usuarios que hacen de relé): salen de la lista si no se vuelven a apuntar
/// en este tiempo (se apuntan cada minuto).
const SUPERNODO_VIVO: Duration = Duration::from_secs(180);
/// Como mucho tantos por IP (una casa con varios móviles) y en total.
const SUPERNODOS_POR_IP: usize = 4;
const MAX_SUPERNODOS: usize = 10_000;
/// Cuántos se dan a quien pregunta (los más recientes).
const SUPERNODOS_DADOS: usize = 50;
/// Buzones que admite un supernodo: lo que anuncie por encima se recorta aquí.
const TOPE_BUZONES: u16 = 64;

/// Un hilo con la pila pequeña. Si el sistema no da más hilos, devuelve el error: se rechaza esa
/// conexión en vez de caerse el relé entero (`thread::spawn` aborta en ese caso).
fn hilo<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> std::io::Result<thread::JoinHandle<T>> {
    thread::Builder::new().stack_size(STACK).spawn(f)
}
const LETTER_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// Como mucho un aviso por buzón cada este tiempo (varias cartas seguidas = un aviso).
const NOTIFY_EVERY: Duration = Duration::from_secs(20);
/// Procesos de aviso vivos a la vez, como máximo. DEPOSITAR no está autenticado y los buzones son
/// gratis (basta una clave Ed25519 nueva), y el límite de `NOTIFY_EVERY` es por buzón: sin un tope
/// global, un atacante acumularía procesos y consumiría la cuota de avisos del operador.
const MAX_AVISOS: usize = 32;

#[derive(Default)]
struct State {
    /// buzón → (conexión de control de su dueño, IP del dueño)
    boxes: HashMap<String, (TcpStream, IpAddr)>,
    /// ficha → (buzón, conexión de quien marcó) esperando a que el dueño acepte
    pending: HashMap<String, (String, TcpStream)>,
    /// conexiones abiertas por IP
    conns: HashMap<IpAddr, usize>,
    /// buzón → cartas (ficha, carta, cuándo llegó, origen del que llegó)
    letters: HashMap<String, Vec<(String, String, std::time::Instant, IpAddr)>>,
    letter_bytes: usize,
    /// Bytes guardados ahora mismo que llegaron de cada origen (para `MAX_LETTER_BYTES_POR_IP`).
    deposit_bytes: HashMap<IpAddr, usize>,
    last_purge: Option<Instant>,
    /// buzón → (ficha de avisos, último aviso)
    push: HashMap<String, (String, Option<std::time::Instant>)>,
    /// supernodo «ip:puerto» → lo que se sabe de él
    supernodos: HashMap<String, Sn>,
}

/// Un supernodo apuntado en el director: cuándo se comprobó que se llega a él desde fuera, su
/// dirección en la wifi de su casa (para quien esté en esa misma red) y cómo está el teléfono que
/// lo presta. Lo último se lo cree el director tal cual: sirve para elegir, no para dar permisos.
struct Sn {
    visto: Instant,
    wifi: Option<String>,
    /// Batería que le queda, en múltiplos de 10 %.
    bat: u8,
    cargando: bool,
    /// Buzones que atiende.
    uso: u16,
}

impl State {
    fn purge_old_letters(&mut self) {
        if self.last_purge.is_some_and(|t| t.elapsed() < PURGE_EVERY) {
            return;
        }
        self.last_purge = Some(Instant::now());
        let mut freed = 0;
        let mut por_ip: Vec<(IpAddr, usize)> = Vec::new();
        for list in self.letters.values_mut() {
            list.retain(|(_, l, at, ip)| {
                let keep = at.elapsed() < LETTER_TTL;
                if !keep {
                    freed += l.len() + LETTER_OVERHEAD;
                    por_ip.push((*ip, l.len() + LETTER_OVERHEAD));
                }
                keep
            });
        }
        for (ip, cost) in por_ip {
            self.release_deposit(ip, cost);
        }
        self.letters.retain(|_, l| !l.is_empty());
        self.letter_bytes -= freed;
    }

    /// Devuelve a un origen la cuota de una carta que ya no se guarda.
    fn release_deposit(&mut self, ip: IpAddr, cost: usize) {
        if let Some(b) = self.deposit_bytes.get_mut(&ip) {
            *b = b.saturating_sub(cost);
            if *b == 0 {
                self.deposit_bytes.remove(&ip);
            }
        }
    }

    /// Guarda una carta si caben (por buzón, por origen y en total). Devuelve si la guardó.
    fn keep_letter(&mut self, id: &str, letter: &str, ip: IpAddr) -> Result<bool> {
        self.purge_old_letters();
        let full = self.letters.get(id).is_some_and(|l| l.len() >= MAX_LETTERS_PER_BOX);
        let cost = letter.len() + LETTER_OVERHEAD;
        let del_origen = self.deposit_bytes.get(&ip).copied().unwrap_or(0);
        if full || self.letter_bytes + cost > MAX_LETTER_BYTES || del_origen + cost > MAX_LETTER_BYTES_POR_IP {
            return Ok(false);
        }
        let mut token = [0u8; 16];
        getrandom::fill(&mut token).map_err(|e| e.to_string())?;
        self.letter_bytes += cost;
        *self.deposit_bytes.entry(ip).or_default() += cost;
        self.letters.entry(id.to_string()).or_default().push((to_hex(&token), letter.to_string(), Instant::now(), ip));
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
    /// Procesos de aviso vivos en este momento (tope: `MAX_AVISOS`).
    avisos: Arc<AtomicUsize>,
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

    /// Apunta el supernodo `ip:puerto` si contesta a QUIEN como un relé. Devuelve si quedó apuntado.
    fn add_supernode(&self, ip: IpAddr, puerto: u16, wifi: Option<String>, bat: u8, cargando: bool, uso: u16) -> bool {
        let addr = std::net::SocketAddr::new(ip, puerto);
        let contesta = TcpStream::connect_timeout(&addr, WAIT).is_ok_and(|mut c| {
            writeln!(c, "QUIEN").is_ok() && read_line(&mut Deadline(&c, Instant::now() + WAIT)).is_ok_and(|l| l.parse::<IpAddr>().is_ok())
        });
        if !contesta {
            return false;
        }
        let mut st = self.lock();
        st.supernodos.retain(|_, sn| sn.visto.elapsed() < SUPERNODO_VIVO);
        let clave = addr.to_string();
        let de_esa_ip = st.supernodos.keys().filter(|k| k.parse::<std::net::SocketAddr>().is_ok_and(|a| a.ip() == ip) && **k != clave).count();
        if !st.supernodos.contains_key(&clave) && (de_esa_ip >= SUPERNODOS_POR_IP || st.supernodos.len() >= MAX_SUPERNODOS) {
            return false;
        }
        st.supernodos.insert(clave, Sn { visto: Instant::now(), wifi, bat: (bat.min(100) / 10) * 10, cargando, uso: uso.min(TOPE_BUZONES) });
        true
    }

    /// Números del relé en JSON (solo cuentas, nada de quién: el relé no guarda registros).
    pub fn estado(&self) -> String {
        let st = self.lock();
        format!(
            r#"{{"usuarios":{},"conexiones":{},"ips":{},"llamadas":{},"esperando":{},"cartas":{},"cartas_bytes":{},"avisos":{},"reenviado":{},"max":{},"supernodos":{}}}"#,
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
            st.supernodos.values().filter(|sn| sn.visto.elapsed() < SUPERNODO_VIVO).count(),
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
            // Director: un dispositivo se ofrece como relé en <puerto> de SU IP (nunca la de otro),
            // y solo entra si desde aquí se le llega y contesta como un relé.
            "SUPERNODO" => {
                let publica = match ip {
                    IpAddr::V4(v4) => !(v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()),
                    IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified() || v6.to_ipv4_mapped().is_some()),
                };
                // «SUPERNODO <puerto> [ip-de-su-wifi:puerto|-] [b=NN] [c=0|1] [u=NN]»: la de la wifi
                // solo si es privada; lo demás es cómo está el teléfono y es opcional (un supernodo
                // antiguo no lo manda).
                let mut partes = arg.split_whitespace();
                let puerto = partes.next().and_then(|p| p.parse::<u16>().ok()).filter(|p| *p > 0);
                let wifi = partes.next().and_then(|w| w.parse::<std::net::SocketAddr>().ok()).filter(|w| match w.ip() {
                    IpAddr::V4(v4) => v4.is_private(),
                    IpAddr::V6(v6) => v6.segments()[0] & 0xfe00 == 0xfc00,
                });
                let (mut bat, mut cargando, mut uso) = (0u8, false, TOPE_BUZONES);
                for t in partes {
                    match t.split_once('=') {
                        Some(("b", v)) => bat = v.parse().unwrap_or(0),
                        Some(("c", v)) => cargando = v == "1",
                        Some(("u", v)) => uso = v.parse().unwrap_or(TOPE_BUZONES),
                        _ => {}
                    }
                }
                let ok = publica && puerto.is_some_and(|p| self.add_supernode(ip, p, wifi.map(|w| w.to_string()), bat, cargando, uso));
                writeln!(s, "{}", if ok { "OK" } else { "NO" })?;
                Ok(())
            }
            // Los dos dan la misma lista; «SUPERNODOS+» añade cómo está cada uno, para que quien
            // pregunte elija por calidad y no al azar. Un cliente antiguo pide «SUPERNODOS» y nada
            // cambia para él.
            "SUPERNODOS" | "SUPERNODOS+" => {
                let con_metricas = cmd == "SUPERNODOS+";
                let mut st = self.lock();
                st.supernodos.retain(|_, sn| sn.visto.elapsed() < SUPERNODO_VIVO);
                let mut lista: Vec<_> = st.supernodos.iter().collect();
                lista.sort_by_key(|(_, sn)| std::cmp::Reverse(sn.visto));
                // «pública» o «pública>wifi»: quien esté en la misma casa usa la de la wifi.
                let lista: Vec<String> = lista
                    .iter()
                    .take(SUPERNODOS_DADOS)
                    .map(|(a, sn)| {
                        let addr = sn.wifi.as_ref().map_or_else(|| a.to_string(), |w| format!("{a}>{w}"));
                        if con_metricas { format!("{addr};b={};c={};u={}", sn.bat, u8::from(sn.cargando), sn.uso) } else { addr }
                    })
                    .collect();
                writeln!(s, "{}", lista.join(","))?;
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
        let mut aviso: Option<String> = None;
        let ok = id.len() == 32
            && !letter.is_empty()
            && letter.len() <= MAX_LETTER
            && letter.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
            && {
                let mut st = self.lock();
                if !st.keep_letter(id, letter, ip_key(s.peer_addr()?.ip()))? {
                    false
                } else {
                    if self.notifier.is_some()
                        && self.avisos.load(Ordering::Relaxed) < MAX_AVISOS
                        && let Some((push, last)) = st.push.get_mut(id)
                        && last.is_none_or(|t| t.elapsed() > NOTIFY_EVERY)
                    {
                        *last = Some(std::time::Instant::now());
                        aviso = Some(push.clone());
                    }
                    true
                }
            };
        // Fuera del mutex: el fork/exec no debe bloquear el relé entero.
        if let (Some(cmd), Some(ficha)) = (&self.notifier, aviso)
            && self
                .avisos
                .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |n| (n < MAX_AVISOS).then_some(n + 1))
                .is_ok()
        {
            // La ficha va como argumento, nunca dentro de una orden de la terminal.
            match std::process::Command::new(cmd).arg(ficha).spawn() {
                Ok(mut hijo) => {
                    let avisos = self.avisos.clone();
                    let contador = avisos.clone();
                    // Si el hilo no se crea, el cierre se descarta sin ejecutarse: se decrementa aquí.
                    if hilo(move || {
                        let _ = hijo.wait();
                        avisos.fetch_sub(1, Ordering::AcqRel);
                    })
                    .is_err()
                    {
                        contador.fetch_sub(1, Ordering::AcqRel);
                    }
                }
                Err(_) => {
                    self.avisos.fetch_sub(1, Ordering::AcqRel);
                }
            }
        }
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
            st.letters.get(&id).map(|l| l.iter().map(|(t, c, _, _)| (t.clone(), c.clone())).collect()).unwrap_or_default()
        };
        for (token, letter) in &letters {
            writeln!(s, "CARTA {token} {letter}")?;
        }
        writeln!(s, "FIN")?;
        while let Ok(line) = read_line(&mut Deadline::of(&s)) {
            let Some(token) = line.strip_prefix("RECIBIDA ") else { break };
            let mut st = self.lock();
            if let Some(list) = st.letters.get_mut(&id)
                && let Some(i) = list.iter().position(|(t, _, _, _)| t == token)
            {
                let (_, letter, _, ip) = list.remove(i);
                st.letter_bytes -= letter.len() + LETTER_OVERHEAD;
                st.release_deposit(ip, letter.len() + LETTER_OVERHEAD);
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
        // Un origen distinto por cada octavo del cupo: así solo choca el tope global, no la cuota por origen.
        let por_origen = super::MAX_LETTER_BYTES_POR_IP / (1 + super::LETTER_OVERHEAD);
        while st.keep_letter(&format!("{n:032}"), "A", std::net::Ipv4Addr::new(10, 0, 0, (n / por_origen) as u8).into()).unwrap() {
            n += 1;
        }
        // Cada carta cuenta su texto y lo que ocupa aparte: el tope llega mucho antes de agotar la memoria.
        assert_eq!(n, super::MAX_LETTER_BYTES / (1 + super::LETTER_OVERHEAD));
    }

    #[test]
    fn un_origen_no_pasa_de_su_cuota_aunque_cambie_de_buzon() {
        let mut st = super::State::default();
        let a: std::net::IpAddr = "1.2.3.4".parse().unwrap();
        let b: std::net::IpAddr = "5.6.7.8".parse().unwrap();
        let carta = "A".repeat(super::MAX_LETTER);
        let mut n = 0;
        while st.keep_letter(&format!("{n:032}"), &carta, a).unwrap() {
            n += 1;
        }
        // El origen se queda en su cuota y no llena el cupo global.
        assert!(n > 0 && st.deposit_bytes[&a] <= super::MAX_LETTER_BYTES_POR_IP);
        assert!(st.letter_bytes < super::MAX_LETTER_BYTES);
        // Otro origen distinto sigue pudiendo depositar.
        assert!(st.keep_letter(&format!("{:032}", 9999), "carta", b).unwrap());
        // Al caducar las cartas del primero recupera su cuota.
        for l in st.letters.values_mut() {
            for c in l.iter_mut().filter(|c| c.3 == a) {
                c.2 -= super::LETTER_TTL + std::time::Duration::from_secs(1);
            }
        }
        st.last_purge = None;
        assert!(st.keep_letter(&format!("{:032}", 10000), "carta", a).unwrap());
        assert_eq!(st.deposit_bytes[&a], "carta".len() + super::LETTER_OVERHEAD);
    }

    #[test]
    fn el_estado_cuenta_sin_decir_quien() {
        let r = super::Relay::default();
        r.lock().keep_letter(&"a".repeat(32), "carta", "1.2.3.4".parse().unwrap()).unwrap();
        let e = r.estado();
        assert!(e.starts_with('{') && e.ends_with('}'), "{e}");
        assert!(e.contains(r#""cartas":1"#) && e.contains(r#""usuarios":0"#), "{e}");
    }

    #[test]
    fn el_director_solo_apunta_supernodos_que_contestan() {
        use std::net::TcpListener;
        // Un relé de verdad en un puerto libre: contesta a QUIEN.
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = l.local_addr().unwrap().port();
        std::thread::spawn(move || super::Relay::default().serve(l));
        let director = super::Relay::default();
        let ip: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        assert!(director.add_supernode(ip, puerto, Some("192.168.1.9:7840".into()), 87, false, 3));
        // Un puerto donde nadie contesta como relé: no entra.
        let mudo = TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(!director.add_supernode(ip, mudo.local_addr().unwrap().port(), None, 0, false, 0));
        assert_eq!(director.lock().supernodos.len(), 1);
        assert_eq!(director.lock().supernodos.values().next().unwrap().wifi.as_deref(), Some("192.168.1.9:7840"));
        assert!(director.estado().contains(r#""supernodos":1"#));
        // La batería se guarda redondeada a múltiplos de 10 %: no sirve de huella del teléfono.
        assert_eq!(director.lock().supernodos.values().next().unwrap().bat, 80);
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
