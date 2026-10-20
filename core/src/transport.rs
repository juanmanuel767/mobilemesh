//! Conexión directa cifrada entre dos dispositivos (TCP + Noise XX).

use crate::{
    net::{Conn, Stream},Identity, Result, identity::{NOISE, verify_cert}};
use serde::{Deserialize, Serialize};
use crate::tiempo::Instant;
use std::{
    io::ErrorKind,
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

/// Versión del protocolo. Va dentro del saludo: dos versiones distintas no llegan a conectar.
const PROLOGUE: &[u8] = b"mobilemesh/2";
const MAX: usize = 65535; // límite de un paquete Noise
const TIMEOUT: Duration = Duration::from_secs(10);
/// Si hay otra ruta (relé) detrás, la directa no espera tanto.
const SHORT: Duration = Duration::from_secs(3);
/// Ventaja de cada ruta sobre la siguiente en la carrera de `Session::connect` (en la misma wifi
/// la directa conecta en milisegundos y el relé ni arranca).
const VENTAJA: Duration = Duration::from_millis(400);
/// Los paquetes se rellenan hasta un múltiplo de este tamaño.
pub const PAD: usize = 512;

/// Abre una conexión siguiendo una ruta: «ip:puerto», «relay=host:puerto/buzón» o varias separadas
/// por «|», que se prueban en orden (primero la directa: el relé solo si hace falta).
pub fn open(route: &str) -> Result<Stream> {
    let parts: Vec<&str> = route.split('|').map(str::trim).filter(|p| !p.is_empty()).collect();
    let wait = if parts.len() > 1 { SHORT } else { TIMEOUT };
    let mut last: Option<crate::Result<Stream>> = None;
    for part in parts {
        let attempt = match part.strip_prefix("relay=") {
            Some(rest) => rest
                .rsplit_once('/')
                .ok_or_else(|| "ruta de relé incompleta".into())
                // Un relé en la wifi de otra casa (supernodo): se salta al momento, como una directa.
                .and_then(|(relay, id)| match relay.parse::<std::net::SocketAddr>() {
                    Ok(a) if !reachable(&a) => Err("relé en una red privada de otra casa".into()),
                    _ => crate::relay::dial(relay, id),
                }),
            None => part
                .to_socket_addrs()
                .map_err(Into::into)
                .and_then(|mut a| a.next().ok_or_else(|| "dirección no válida".into()))
                .and_then(|a| match reachable(&a) {
                    true => TcpStream::connect_timeout(&a, wait)
                        .and_then(|t| t.set_nodelay(true).map(|_| Box::new(t) as Stream))
                        .map_err(Into::into),
                    false => Err("dirección privada de otra red".into()),
                }),
        };
        if attempt.is_ok() {
            return attempt;
        }
        last = Some(attempt);
    }
    last.unwrap_or_else(|| Err("no hay ninguna ruta para llegar a ese dispositivo".into()))
}

/// ¿Merece la pena intentar esta dirección directa? Una privada (wifi de casa, red de la operadora)
/// solo se alcanza desde la misma red: si mi dirección de salida hacia ella no está en su mismo
/// bloque /16, se salta y se va directo al relé (con datos móviles se perdían segundos esperando).
// ponytail: compara el /16; una red privada más grande que eso se trataría como «otra red».
pub(crate) fn reachable(target: &std::net::SocketAddr) -> bool {
    // IPv6: solo si tengo ruta hacia ella (sin IPv6, se salta al momento en vez de esperar).
    let std::net::IpAddr::V4(t) = target.ip() else {
        return std::net::UdpSocket::bind("[::]:0").and_then(|u| u.connect(target)).is_ok();
    };
    let private = t.is_private() || t.is_loopback() || (t.octets()[0] == 100 && (64..128).contains(&t.octets()[1]));
    if !private || t.is_loopback() {
        return true;
    }
    let mine = std::net::UdpSocket::bind("0.0.0.0:0").and_then(|u| u.connect(target).and(u.local_addr()));
    match mine.map(|a| a.ip()) {
        Ok(std::net::IpAddr::V4(m)) => m.octets()[..2] == t.octets()[..2],
        _ => true,
    }
}

/// ¿«host:puerto» válido? En móvil y PC se resuelve el nombre; en el navegador no se puede buscar
/// un nombre en internet sin conectar, así que solo se comprueba la forma (lo encontró una app de
/// prueba: con el relé por nombre de DuckDNS no se podían añadir contactos desde la web).
fn valid_addr(a: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return a.to_socket_addrs().is_ok();
    #[cfg(target_arch = "wasm32")]
    a.rsplit_once(':').is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok())
}

/// Comprueba que una ruta tiene buen formato (sin conectar).
pub fn check_route(route: &str) -> Result<()> {
    for part in route.split('|').map(str::trim) {
        match part.strip_prefix("relay=") {
            Some(rest) => {
                let (relay, id) = rest.rsplit_once('/').ok_or("ruta de relé incompleta")?;
                if !valid_addr(relay) {
                    return Err("dirección del relé no válida".into());
                }
                if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("buzón del relé no válido".into());
                }
            }
            None if !valid_addr(part) => return Err("la dirección no es válida".into()),
            None => {}
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Frame {
    /// Pide las claves para abrir una sesión cifrada (cuando aún no hay ninguna con ese dispositivo).
    KeysReq,
    Keys { identity: String, one_time: String },
    /// Mensaje cifrado con el ratchet: tipo Olm y texto cifrado en base64.
    Msg { id: String, kind: usize, body: String },
    /// El destino no tiene sesión con nosotros (la perdió): hay que abrir otra.
    NoSession { id: String },
    /// Respuesta a un archivo: cuántos trozos tiene ya el destino (para continuar desde ahí).
    Have { id: String, chunks: u64 },
    /// Un trozo del archivo, ya cifrado con la clave del archivo, en base64.
    Chunk { id: String, index: u64, data: String },
    /// El destino rechaza el archivo (no supera la comprobación de integridad o es demasiado grande).
    Bad { id: String, reason: String },
    /// Sincronización entre dispositivos de la misma cuenta (contenido cifrado con el ratchet).
    Sync { id: String, kind: usize, body: String },
    /// Copia de seguridad (Fase 8): «guárdame esta copia» o, al recuperar, «aquí está tu copia».
    /// `meta` va cifrado con la clave de la cuenta del dueño: quien la guarda no puede leerlo.
    ReplicaPut { version: u64, meta: String, size: u64 },
    /// «Demuéstrame que tienes mi copia entera»: huella de (este número + la copia).
    ReplicaCheck { nonce: String },
    ReplicaProof { version: u64, proof: String },
    /// «Devuélveme mi copia» (la cuenta se demuestra en el saludo).
    ReplicaGet,
    /// «Borra mi copia».
    ReplicaDel,
    /// Al empezar cada conexión: lista firmada de dispositivos retirados de mi cuenta (si hay).
    Rev { list: Option<String> },
    /// Confirmación: el destino ya lo guardó en su base de datos.
    Ack { id: String },
    /// Guardián: «guarda mi código de recuperación en este localizador» / «dame el de este
    /// localizador» / respuesta (ninguno si no existe).
    CuentaPut { id: String, code: String },
    CuentaGet { id: String },
    Cuenta { code: Option<String> },
    /// Guía de nombres: «registra mi nombre» / «¿invitación de este nombre?» / respuesta /
    /// «avisa a este nombre de que quiero hablar con él» / «¿quién quiere hablar conmigo?» / lista.
    /// `app`: el espacio de nombres de cada app (vacío = el común, el de los clientes viejos).
    NombrePut {
        #[serde(default)]
        app: String,
        nombre: String,
        invitacion: String,
    },
    NombreGet {
        #[serde(default)]
        app: String,
        nombre: String,
    },
    Nombre { invitacion: Option<String> },
    NombreSolicitar {
        #[serde(default)]
        app: String,
        nombre: String,
    },
    NombreSolicitudes,
    Solicitudes { lista: Vec<(String, String)> },
    /// Colecciones públicas: publicar / borrar lo mío / consultar lo de todos / respuesta.
    PublicoPut { app: String, coleccion: String, id: String, campos: String },
    PublicoBorrar { app: String, coleccion: String, id: String },
    PublicoConsulta {
        app: String,
        coleccion: String,
        filtros: Vec<(String, String, String)>,
        orden: Option<String>,
        desc: bool,
        limite: u32,
    },
    PublicoLista { docs: Vec<(String, String, String)> },
}

pub struct Session {
    stream: Stream,
    noise: snow::TransportState,
    /// Cuenta del otro lado.
    pub peer: Vec<u8>,
    /// Clave del dispositivo concreto del otro lado.
    pub device: Vec<u8>,
    /// Plazo TOTAL para recibir cada paquete (no por byte: así un atacante no puede retener la
    /// conexión mandando un byte cada pocos segundos; auditoría, Fase 13).
    wait: Duration,
}

/// Plazo total para completar el saludo y para recibir cada paquete.
const HANDSHAKE: Duration = Duration::from_secs(15);
const FRAME: Duration = Duration::from_secs(30);

fn builder(me: &Identity) -> Result<snow::Builder<'_>> {
    Ok(snow::Builder::new(NOISE.parse()?).local_private_key(&me.private)?.prologue(PROLOGUE)?)
}

/// Tamaño + paquete en UNA sola escritura: en dos, TCP retenía el segundo trozo esperando la
/// confirmación del primero (algoritmo de Nagle + ACK retardado), ≈ 40 ms por paquete (medido).
fn write_raw(s: &mut dyn Conn, b: &[u8]) -> Result<()> {
    let mut paquete = Vec::with_capacity(2 + b.len());
    paquete.extend_from_slice(&(b.len() as u16).to_be_bytes());
    paquete.extend_from_slice(b);
    s.write_all(&paquete)?;
    Ok(())
}

/// Llena `buf` antes de `deadline`. `Ok(false)` si el otro lado cerró antes del primer byte.
fn read_until(s: &mut dyn Conn, buf: &mut [u8], deadline: Instant) -> Result<bool> {
    let mut n = 0;
    while n < buf.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("se agotó el tiempo de espera".into());
        }
        s.set_read_timeout(Some(left))?;
        match s.read(&mut buf[n..]) {
            Ok(0) if n == 0 => return Ok(false),
            Ok(0) => return Err(std::io::Error::from(ErrorKind::UnexpectedEof).into()),
            Ok(k) => n += k,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(true)
}

/// `None` si el otro lado cerró la conexión.
fn read_raw(s: &mut dyn Conn, deadline: Instant) -> Result<Option<Vec<u8>>> {
    let mut len = [0u8; 2];
    if !read_until(s, &mut len, deadline)? {
        return Ok(None);
    }
    let mut b = vec![0; u16::from_be_bytes(len) as usize];
    if !read_until(s, &mut b, deadline)? && !b.is_empty() {
        return Err(std::io::Error::from(ErrorKind::UnexpectedEof).into());
    }
    Ok(Some(b))
}

/// Lee la prueba del otro lado (cuenta + firma sobre su clave de dispositivo) y devuelve su cuenta.
/// Va cifrada dentro del saludo Noise, así que un espía no ve a qué cuenta pertenece cada dispositivo.
fn check_proof(proof: &[u8], device: Option<&[u8]>) -> Result<Vec<u8>> {
    let device = device.ok_or("el otro lado no envió su clave")?;
    if proof.len() != 96 {
        return Err("el otro lado no envió un certificado válido".into());
    }
    verify_cert(&proof[..32], device, &proof[32..])?;
    Ok(proof[..32].to_vec())
}

fn read_hs(s: &mut dyn Conn, deadline: Instant) -> Result<Vec<u8>> {
    read_raw(s, deadline)?.ok_or_else(|| "conexión cerrada durante el saludo".into())
}

impl Session {
    /// Conecta y exige que el otro dispositivo demuestre pertenecer a la cuenta `expect`. Con varias
    /// rutas («directa|relay=…») compiten escalonadas: la directa sale primero y cada siguiente
    /// arranca `VENTAJA` después; gana la primera que completa el saludo. Si la directa la contesta
    /// otro dispositivo (otra red con la misma IP privada), el relé sigue en carrera.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn connect(route: &str, me: &Identity, expect: &[u8]) -> Result<Session> {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc};
        let parts: Vec<String> = route.split('|').map(str::trim).filter(|p| !p.is_empty()).map(String::from).collect();
        if parts.len() <= 1 {
            return Self::connect_one(route, me, expect);
        }
        let (tx, rx) = mpsc::channel();
        let ganada = Arc::new(AtomicBool::new(false));
        let total = parts.len();
        for (i, part) in parts.into_iter().enumerate() {
            let (tx, me, expect, ganada) = (tx.clone(), me.clone(), expect.to_vec(), ganada.clone());
            std::thread::spawn(move || {
                std::thread::sleep(VENTAJA * i as u32);
                let r = if ganada.load(Ordering::SeqCst) { Err("otra ruta ya conectó".into()) } else { Self::connect_one(&part, &me, &expect) };
                if r.is_ok() {
                    ganada.store(true, Ordering::SeqCst);
                }
                let _ = tx.send(r); // si ya ganó otra, esta sesión se suelta y se cierra
            });
        }
        drop(tx);
        let mut ultimo = None;
        for r in rx.iter().take(total) {
            match r {
                Ok(s) => return Ok(s),
                Err(e) => ultimo = Some(e),
            }
        }
        Err(ultimo.unwrap_or_else(|| "no hay ninguna ruta para llegar a ese dispositivo".into()))
    }

    /// En el navegador no hay hilos: rutas en orden.
    #[cfg(target_arch = "wasm32")]
    pub fn connect(route: &str, me: &Identity, expect: &[u8]) -> Result<Session> {
        let mut ultimo = None;
        for part in route.split('|').map(str::trim).filter(|p| !p.is_empty()) {
            match Self::connect_one(part, me, expect) {
                Ok(s) => return Ok(s),
                Err(e) => ultimo = Some(e),
            }
        }
        Err(ultimo.unwrap_or_else(|| "no hay ninguna ruta para llegar a ese dispositivo".into()))
    }

    /// Conecta por una sola ruta y hace el saludo.
    fn connect_one(route: &str, me: &Identity, expect: &[u8]) -> Result<Session> {
        let mut stream = open(route)?;
        let deadline = Instant::now() + HANDSHAKE;
        let mut hs = builder(me)?.build_initiator()?;
        let mut buf = vec![0; MAX];
        let n = hs.write_message(&[], &mut buf)?;
        write_raw(&mut stream, &buf[..n])?;
        let n = hs.read_message(&read_hs(&mut stream, deadline)?, &mut buf)?;
        // Comprobado antes de enviar nuestra identidad: a un impostor no le revelamos quiénes somos.
        let device = hs.get_remote_static().ok_or("el otro lado no envió su clave")?.to_vec();
        let account = check_proof(&buf[..n], Some(&device))?;
        if account != expect {
            return Err("el destino no pertenece a la cuenta esperada: posible suplantación".into());
        }
        let n = hs.write_message(&me.proof(), &mut buf)?;
        write_raw(&mut stream, &buf[..n])?;
        Ok(Session { stream, noise: hs.into_transport_mode()?, peer: account, device, wait: FRAME })
    }

    /// Acepta una conexión entrante. `peer` es la cuenta del otro lado; quien llama decide si es de confianza.
    pub fn accept(stream: impl Conn + 'static, me: &Identity) -> Result<Session> {
        let mut stream: Stream = Box::new(stream);
        let deadline = Instant::now() + HANDSHAKE;
        let mut hs = builder(me)?.build_responder()?;
        let mut buf = vec![0; MAX];
        hs.read_message(&read_hs(&mut stream, deadline)?, &mut buf)?;
        let n = hs.write_message(&me.proof(), &mut buf)?;
        write_raw(&mut stream, &buf[..n])?;
        let n = hs.read_message(&read_hs(&mut stream, deadline)?, &mut buf)?;
        let device = hs.get_remote_static().ok_or("el otro lado no envió su clave")?.to_vec();
        let peer = check_proof(&buf[..n], Some(&device))?;
        Ok(Session { stream, noise: hs.into_transport_mode()?, peer, device, wait: FRAME })
    }

    /// Cambia cuánto se espera una respuesta (por ejemplo, mientras el otro comprueba un archivo grande).
    pub fn set_timeout(&mut self, t: Duration) -> Result<()> {
        self.wait = if t.is_zero() { FRAME } else { t.max(FRAME) };
        Ok(())
    }

    pub fn send(&mut self, f: &Frame) -> Result<()> {
        let json = serde_json::to_vec(f)?;
        // Relleno hasta un múltiplo de PAD: por el tamaño ya no se adivina qué se escribió.
        let len = (4 + json.len()).div_ceil(PAD) * PAD;
        if len > MAX - 16 {
            return Err("mensaje demasiado grande".into());
        }
        let mut plain = vec![0u8; len];
        plain[..4].copy_from_slice(&(json.len() as u32).to_be_bytes());
        plain[4..4 + json.len()].copy_from_slice(&json);
        let mut buf = vec![0; MAX];
        let n = self.noise.write_message(&plain, &mut buf)?;
        write_raw(&mut self.stream, &buf[..n])
    }

    /// Descifra el siguiente paquete. Un paquete alterado, repetido o reordenado da error.
    pub fn recv(&mut self) -> Result<Option<Frame>> {
        let Some(c) = read_raw(&mut self.stream, Instant::now() + self.wait)? else { return Ok(None) };
        let mut buf = vec![0; MAX];
        let n = self.noise.read_message(&c, &mut buf)?;
        let len = u32::from_be_bytes(buf[..4.min(n)].try_into().map_err(|_| "paquete demasiado corto")?) as usize;
        let end = 4usize.checked_add(len).filter(|e| *e <= n).ok_or("paquete con relleno incorrecto")?;
        let json = &buf[4..end];
        Ok(Some(serde_json::from_slice(json)?))
    }
}
