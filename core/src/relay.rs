//! Relé (Fase 9): une dos conexiones cuando los dispositivos no pueden conectarse directamente
//! (redes distintas, datos móviles, routers). Cualquiera puede ejecutar uno (`mesh-rele`).
//!
//! - Cada dispositivo mantiene una conexión **saliente** con el relé registrando un **buzón**: un
//!   identificador que no tiene que ver con su cuenta. El buzón es la huella de una clave pública;
//!   para registrarlo hay que firmar un número al azar que manda el relé, así que **el secreto nunca
//!   viaja** y quien espíe la red no puede quedarse con el buzón (auditoría, Fase 13).
//! - Quien quiere llamar pide al relé ese buzón; el relé avisa al dueño, que abre otra conexión, y
//!   el relé **empalma** las dos. Por ellas va el saludo Noise de extremo a extremo: el relé solo
//!   ve bytes cifrados.
//! - El relé no guarda nada ni escribe registros. Sí ve: direcciones IP, horas, tamaños y qué
//!   buzón habla con qué conexión.
//! - Límites contra la saturación: conexiones en total y por IP, buzones por IP y llamadas en espera
//!   por buzón; la conexión de control se cierra si deja de dar señales.
//!
//! - **Buzón de cartas** (para quien está dormido): cualquiera puede dejar una carta cifrada de
//!   extremo a extremo en un buzón; solo el dueño (con la misma firma) puede recogerlas. El relé no
//!   puede leerlas, las guarda en memoria como mucho 7 días y las borra al confirmarse.
//!
//! Protocolo de control: líneas de texto al empezar cada conexión.
//!   REGISTRAR <clave pública> → RETO <número> → FIRMA <firma> → OK <buzón>
//!                               (y después: LLAMADA <ficha> por cada llamada)
//!   MARCAR <buzón>            → OK | NO      (con OK, la conexión queda empalmada)
//!   ACEPTAR <ficha>           → (la conexión queda empalmada con quien marcó)
//!   DEPOSITAR <buzón> <carta> → OK | NO
//!   RECOGER <clave pública>   → RETO → FIRMA → CARTA <ficha> <carta>… FIN; el dueño contesta
//!                               RECIBIDA <ficha> por cada una y la borra el relé
//!   AVISO <clave pública>     → RETO → FIRMA → <ficha de avisos o «-» para quitarla> → OK
//!   DIRECTORIO                → <directorio firmado> | NO
//!   QUIEN                     → <la IP con la que el relé me ve>
//!
//! **Avisos (opcional):** si el relé se arranca con un programa de avisos (p. ej. el que avisa por
//! Firebase), al llegar una carta a un buzón con ficha de avisos lo ejecuta con la ficha como
//! argumento. El aviso no lleva contenido: solo despierta al teléfono para que recoja su buzón.

/// Relés que usa toda app que no elija los suyos: así funciona entre redes distintas sin
/// configurar nada. Se prueban en orden y el buzón se registra en todos.
// ponytail: lista fija en el programa; un directorio de relés que se actualice solo si crece mucho.
// El primero es el nubia de casa (app «Relé MobileMesh»); cambiar la IP por un nombre de DuckDNS
// cuando exista, porque la de casa puede cambiar.
pub const UNIVERSALES: &[&str] = &["181.60.197.125:7900"];

/// Los 2 relés de `all` que usa el buzón con este secreto: cada teléfono usa solo 2, así la carga se
/// reparte entre todos los de la lista.
pub fn choose(all: &[&str], secret: &str) -> Vec<String> {
    let h = Sha256::digest(secret.as_bytes());
    let start = (u64::from_le_bytes(h[..8].try_into().unwrap_or_default()) % all.len().max(1) as u64) as usize;
    (0..all.len().min(2)).map(|i| all[(start + i) % all.len()].to_string()).collect()
}

/// Valor del ajuste «rele» que significa «ningún relé, ni los universales».
pub const SIN_RELE: &str = "ninguno";

use crate::{
    Result,
    net::Conn,
    identity::{from_hex, from_hex_any, to_hex},
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(10);
/// La conexión de control manda «PING» cada 25 s; si calla más que esto, se cierra.
const SILENCE: Duration = Duration::from_secs(90);
/// Una conexión empalmada sin tráfico durante este tiempo se cierra.
const IDLE: Duration = Duration::from_secs(300);
const CONTEXT: &[u8] = b"mobilemesh-buzon-v1";
// ponytail: límites fijos; hacerlos configurables cuando haya relés públicos con mucha gente.
const MAX_CONNECTIONS: usize = 1024;
// Altos a propósito: con datos móviles cientos de teléfonos salen por la misma IP de la operadora.
const MAX_PER_IP: usize = 1000;
const MAX_BOXES_PER_IP: usize = 1000;
const MAX_PENDING: usize = 256;
const MAX_PENDING_PER_BOX: usize = 4;
/// Cartas: tamaño máximo, cuántas por buzón, total en memoria y cuánto se guardan.
pub const MAX_LETTER: usize = 48 * 1024;
const MAX_LETTERS_PER_BOX: usize = 200;
const MAX_LETTER_BYTES: usize = 64 * 1024 * 1024;
const LETTER_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// Como mucho un aviso por buzón cada este tiempo (varias cartas seguidas = un aviso).
const NOTIFY_EVERY: Duration = Duration::from_secs(20);

/// Lee una línea sin leer de más (lo que venga detrás es tráfico Noise del otro lado).
pub fn read_line(s: &mut impl Read) -> Result<String> {
    read_line_max(s, 200)
}

fn read_line_max(s: &mut impl Read, max: usize) -> Result<String> {
    let mut line = Vec::new();
    let mut b = [0u8; 1];
    while line.len() < max {
        if s.read(&mut b)? == 0 {
            return Err("el relé cerró la conexión".into());
        }
        if b[0] == b'\n' {
            return Ok(String::from_utf8(line)?);
        }
        line.push(b[0]);
    }
    Err("línea demasiado larga".into())
}

/// Clave de firma de un buzón, sacada de su secreto.
fn mailbox_key(secret: &str) -> SigningKey {
    SigningKey::from_bytes(&Sha256::new_with_prefix(CONTEXT).chain_update(secret.as_bytes()).finalize().into())
}

fn id_of(public: &[u8]) -> String {
    to_hex(&Sha256::digest(public)[..16])
}

/// Identificador público de un buzón a partir de su secreto.
pub fn mailbox_id(secret: &str) -> String {
    id_of(&mailbox_key(secret).verifying_key().to_bytes())
}

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
    /// buzón → (ficha de avisos, último aviso)
    push: HashMap<String, (String, Option<std::time::Instant>)>,
}

impl State {
    fn purge_old_letters(&mut self) {
        let mut freed = 0;
        for list in self.letters.values_mut() {
            list.retain(|(_, l, at)| {
                let keep = at.elapsed() < LETTER_TTL;
                if !keep {
                    freed += l.len();
                }
                keep
            });
        }
        self.letters.retain(|_, l| !l.is_empty());
        self.letter_bytes -= freed;
    }
}

/// Un relé en marcha. `forwarded` cuenta los bytes reenviados (para medir lo que cuesta mantenerlo).
#[derive(Clone, Default)]
pub struct Relay {
    state: Arc<Mutex<State>>,
    pub forwarded: Arc<AtomicU64>,
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
            let Ok(ip) = stream.peer_addr().map(|a| a.ip()) else { continue };
            {
                let mut st = self.lock();
                let total: usize = st.conns.values().sum();
                let mine = st.conns.get(&ip).copied().unwrap_or(0);
                if total >= self.max.unwrap_or(MAX_CONNECTIONS) || mine >= MAX_PER_IP {
                    continue; // se cierra sin contestar
                }
                st.conns.insert(ip, mine + 1);
            }
            let me = self.clone();
            thread::spawn(move || {
                let _ = me.handle(stream, ip);
                let mut st = me.lock();
                if let Some(n) = st.conns.get_mut(&ip) {
                    *n -= 1;
                    if *n == 0 {
                        st.conns.remove(&ip);
                    }
                }
            });
        }
    }

    fn handle(&self, mut s: TcpStream, ip: IpAddr) -> Result<()> {
        s.set_read_timeout(Some(WAIT))?;
        s.set_write_timeout(Some(WAIT))?;
        let line = read_line_max(&mut s, MAX_LETTER + 200)?;
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
        let answer = read_line(s)?;
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
                st.purge_old_letters();
                let full = st.letters.get(id).is_some_and(|l| l.len() >= MAX_LETTERS_PER_BOX);
                if full || st.letter_bytes + letter.len() > MAX_LETTER_BYTES {
                    false
                } else {
                    let mut token = [0u8; 16];
                    getrandom::fill(&mut token).map_err(|e| e.to_string())?;
                    st.letter_bytes += letter.len();
                    st.letters.entry(id.to_string()).or_default().push((to_hex(&token), letter.to_string(), std::time::Instant::now()));
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
        let token = read_line_max(&mut s, 4200)?;
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
        while let Ok(line) = read_line(&mut s) {
            let Some(token) = line.strip_prefix("RECIBIDA ") else { break };
            let mut st = self.lock();
            if let Some(list) = st.letters.get_mut(&id)
                && let Some(i) = list.iter().position(|(t, _, _)| t == token)
            {
                let (_, letter, _) = list.remove(i);
                st.letter_bytes -= letter.len();
            }
        }
        Ok(())
    }

    /// Registro del buzón para recibir llamadas.
    fn register(&self, mut s: TcpStream, ip: IpAddr, public: &str) -> Result<()> {
        let id = self.prove(&mut s, public)?;
        {
            let mut st = self.lock();
            if st.boxes.values().filter(|(_, owner_ip)| *owner_ip == ip).count() >= MAX_BOXES_PER_IP {
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
            thread::spawn(move || {
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
            })
        };
        if let (Ok(a2), Ok(b2)) = (a.try_clone(), b.try_clone()) {
            let x = pipe(a, b2, self.forwarded.clone());
            let y = pipe(b, a2, self.forwarded.clone());
            let _ = (x.join(), y.join());
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

/// Mi IP pública tal como la ve el relé.
pub fn public_ip(relay: &str) -> Result<String> {
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "QUIEN")?;
    let ip: IpAddr = read_line(&mut s)?.parse().map_err(|_| "el relé no dio una IP")?;
    Ok(ip.to_string())
}

/// Pide al relé el directorio firmado que sirve (la firma se comprueba fuera).
pub fn directory(relay: &str) -> Result<String> {
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "DIRECTORIO")?;
    match read_line_max(&mut s, 8192)?.as_str() {
        "NO" => Err("el relé no tiene directorio".into()),
        l => Ok(l.to_string()),
    }
}

/// Llama al buzón `id` a través del relé. Devuelve la conexión ya empalmada con su dueño.
pub fn dial(relay: &str, id: &str) -> Result<crate::net::Stream> {
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT + Duration::from_secs(2)))?;
    writeln!(s, "MARCAR {id}")?;
    match read_line(&mut s)?.as_str() {
        "OK" => Ok(s),
        _ => Err("el destinatario no está conectado al relé".into()),
    }
}

/// Deja una carta (ya cifrada de extremo a extremo, en base64) en el buzón `id`.
pub fn deposit(relay: &str, id: &str, letter: &str) -> Result<()> {
    if letter.len() > MAX_LETTER {
        return Err("carta demasiado grande".into());
    }
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "DEPOSITAR {id} {letter}")?;
    match read_line(&mut s)?.as_str() {
        "OK" => Ok(()),
        _ => Err("el relé no aceptó la carta (buzón lleno)".into()),
    }
}

/// Guarda en el relé la ficha de avisos (p. ej. de Firebase) del buzón de `secret`, o la quita con `None`.
pub fn set_push_token(relay: &str, secret: &str, token: Option<&str>) -> Result<()> {
    let key = mailbox_key(secret);
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "AVISO {}", to_hex(&key.verifying_key().to_bytes()))?;
    let challenge = read_line(&mut s)?;
    let nonce = from_hex_any(challenge.strip_prefix("RETO ").ok_or("el relé no mandó el reto")?)?;
    writeln!(s, "FIRMA {}", to_hex(&key.sign(&[CONTEXT, &nonce].concat()).to_bytes()))?;
    writeln!(s, "{}", token.unwrap_or("-"))?;
    match read_line(&mut s)?.as_str() {
        "OK" => Ok(()),
        _ => Err("el relé no aceptó la ficha de avisos".into()),
    }
}

/// Recoge las cartas del buzón de `secret`. `handle` recibe cada una y devuelve si se puede borrar.
/// Devuelve cuántas había.
pub fn collect(relay: &str, secret: &str, mut handle: impl FnMut(&str) -> bool) -> Result<usize> {
    let key = mailbox_key(secret);
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "RECOGER {}", to_hex(&key.verifying_key().to_bytes()))?;
    let challenge = read_line(&mut s)?;
    let nonce = from_hex_any(challenge.strip_prefix("RETO ").ok_or("el relé no mandó el reto")?)?;
    writeln!(s, "FIRMA {}", to_hex(&key.sign(&[CONTEXT, &nonce].concat()).to_bytes()))?;
    let mut n = 0;
    loop {
        let line = read_line_max(&mut s, MAX_LETTER + 100)?;
        if line == "FIN" {
            break;
        }
        let Some((token, letter)) = line.strip_prefix("CARTA ").and_then(|r| r.split_once(' ')) else { break };
        n += 1;
        if handle(letter) {
            writeln!(s, "RECIBIDA {token}")?;
        }
    }
    let _ = writeln!(s, "ADIOS");
    Ok(n)
}

/// Registra el buzón de `secret` en el relé. Devuelve la conexión de control, por la que llegan
/// las líneas «LLAMADA <ficha>».
pub fn register(relay: &str, secret: &str) -> Result<crate::net::Stream> {
    let key = mailbox_key(secret);
    let mut control = crate::net::connect(relay, WAIT)?;
    control.set_read_timeout(Some(WAIT))?;
    writeln!(control, "REGISTRAR {}", to_hex(&key.verifying_key().to_bytes()))?;
    let challenge = read_line(&mut control)?;
    let nonce = from_hex_any(challenge.strip_prefix("RETO ").ok_or("el relé no mandó el reto")?)?;
    writeln!(control, "FIRMA {}", to_hex(&key.sign(&[CONTEXT, &nonce].concat()).to_bytes()))?;
    if !read_line(&mut control)?.starts_with("OK ") {
        return Err("el relé no aceptó el buzón".into());
    }
    Ok(control)
}

/// Contesta la llamada `token`: devuelve la conexión ya empalmada con quien llama.
pub fn answer(relay: &str, token: &str) -> Result<crate::net::Stream> {
    let mut s = crate::net::connect(relay, WAIT)?;
    writeln!(s, "ACEPTAR {token}")?;
    Ok(s)
}

/// Mantiene el buzón registrado en el relé y entrega cada llamada a `on_call`.
/// Si se cae, vuelve a conectar (esperando cada vez más, hasta 1 minuto). Nunca termina.
pub fn listen(relay: String, secret: String, on_call: impl Fn(crate::net::Stream) + Send + Sync + 'static) {
    let on_call = Arc::new(on_call);
    let mut pause = Duration::from_secs(2);
    loop {
        let session = (|| -> Result<()> {
            let mut control = register(&relay, &secret)?;
            pause = Duration::from_secs(2);
            // Un «PING» de vez en cuando mantiene abierta la conexión en routers y redes móviles.
            let mut ping = control.try_clone_conn()?;
            thread::spawn(move || {
                while writeln!(ping, "PING").is_ok() {
                    thread::sleep(Duration::from_secs(25));
                }
            });
            control.set_read_timeout(None)?;
            loop {
                let line = read_line(&mut control)?;
                let Some(token) = line.strip_prefix("LLAMADA ") else { continue };
                let (relay, token, on_call) = (relay.clone(), token.to_string(), on_call.clone());
                thread::spawn(move || {
                    if let Ok(s) = answer(&relay, &token) {
                        on_call(s);
                    }
                });
            }
        })();
        let _ = session;
        thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_secs(60));
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
}
