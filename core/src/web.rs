//! Versión para navegador (pieza aparte, solo WebAssembly). Las conexiones son WebSocket hacia el
//! puente del relé (puerto del relé + 1); el núcleo las usa igual que TCP gracias a `js/conexion.js`.

use crate::net::{Conn, Stream};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/conexion.js")]
extern "C" {
    fn mm_abrir(direccion: &str, ms: u32) -> i32;
    fn mm_leer(id: i32, buf: &mut [u8], ms: u32) -> i32;
    fn mm_escribir(id: i32, datos: &[u8]) -> i32;
    fn mm_cerrar(id: i32);
}

/// El puente WebSocket del relé escucha en el puerto siguiente al del relé.
pub const PUERTO_WEB_EXTRA: u16 = 1;

struct Abierta(i32);

impl Drop for Abierta {
    fn drop(&mut self) {
        mm_cerrar(self.0);
    }
}

// SAFETY: el núcleo en el navegador corre en un solo trabajador; la conexión solo es un número.
unsafe impl Send for Abierta {}
unsafe impl Sync for Abierta {}

pub struct WsConn {
    id: Arc<Abierta>,
    espera_ms: AtomicU32,
}

impl io::Read for WsConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match mm_leer(self.id.0, buf, self.espera_ms.load(Ordering::Relaxed)) {
            n if n >= 0 => Ok(n as usize),
            -1 => Err(io::ErrorKind::TimedOut.into()),
            _ => Err(io::ErrorKind::ConnectionReset.into()),
        }
    }
}

impl io::Write for WsConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match mm_escribir(self.id.0, buf) {
            n if n >= 0 => Ok(n as usize),
            _ => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Conn for WsConn {
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        self.espera_ms.store(t.map_or(0, |d| d.as_millis().clamp(1, u32::MAX as u128) as u32), Ordering::Relaxed);
        Ok(())
    }

    fn try_clone_conn(&self) -> io::Result<Stream> {
        Ok(Box::new(WsConn { id: self.id.clone(), espera_ms: AtomicU32::new(self.espera_ms.load(Ordering::Relaxed)) }))
    }
}

/// «host:puerto» del relé → WebSocket a «host:puerto+1».
pub fn connect(addr: &str, wait: Duration) -> crate::Result<Stream> {
    let (host, port) = addr.rsplit_once(':').ok_or("dirección no válida")?;
    let port: u16 = port.parse().map_err(|_| "puerto no válido")?;
    let id = mm_abrir(&format!("{host}:{}", port + PUERTO_WEB_EXTRA), wait.as_millis() as u32);
    if id < 0 {
        return Err(format!("no se pudo conectar con {host} desde el navegador").into());
    }
    Ok(Box::new(WsConn { id: Arc::new(Abierta(id)), espera_ms: AtomicU32::new(0) }))
}

// ---------- Lo que ve JavaScript ----------

/// Carpeta (virtual) de MobileMesh dentro del almacén privado del navegador.
const DIR: &str = "/mobilemesh";

fn js(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Prepara el almacén permanente del navegador (OPFS). Llamar una vez al empezar.
#[wasm_bindgen]
pub async fn preparar() -> Result<(), JsError> {
    use sqlite_wasm_vfs::sahpool::{OpfsSAHPoolCfg, install};
    install::<sqlite_wasm_rs::WasmOsCallback>(&OpfsSAHPoolCfg::default(), true).await.map_err(|e| js(format!("{e:?}")))?;
    Ok(())
}

// ponytail: en el navegador la identidad se guarda sin envolver (el almacén ya es privado de la
// web); envolverla con una clave no exportable de WebCrypto más adelante.
fn abrir() -> crate::Result<(crate::Identity, crate::Store)> {
    let dir = std::path::Path::new(DIR);
    let me = match crate::Identity::load(dir, &crate::identity::Plain) {
        Ok(me) => me,
        Err(_) => crate::Identity::create(dir, &crate::identity::Plain)?,
    };
    let store = crate::Store::open(dir, me.db_key())?;
    Ok((me, store))
}

/// Abre la cuenta de este navegador (la crea la primera vez). Devuelve su identificador.
#[wasm_bindgen]
pub fn cuenta() -> Result<String, JsError> {
    Ok(abrir().map_err(js)?.0.id())
}

/// Guarda un campo de un registro en una colección (cifrado en el almacén del navegador).
#[wasm_bindgen]
pub fn guardar_dato(coleccion: &str, registro: &str, campo: &str, valor: &str) -> Result<(), JsError> {
    let (me, store) = abrir().map_err(js)?;
    let device = crate::identity::to_hex(&me.public);
    crate::sync::put_as(&store, &device, &me.id(), crate::sync::Policy::Lww, coleccion, registro, campo, Some(valor)).map_err(js)
}

/// Registros de una colección en JSON: `[{"id": …, "campos": {…}}]`.
#[wasm_bindgen]
pub fn ver_datos(coleccion: &str) -> Result<String, JsError> {
    let (_, store) = abrir().map_err(js)?;
    let rows: Vec<serde_json::Value> = crate::sync::records(&store, coleccion)
        .map_err(js)?
        .into_iter()
        .map(|r| serde_json::json!({"id": r.id, "campos": r.fields.into_iter().map(|(k, v)| (k, serde_json::Value::String(v))).collect::<serde_json::Map<_, _>>()}))
        .collect();
    serde_json::to_string(&rows).map_err(js)
}

/// Mi IP pública según el relé (prueba de que el núcleo habla por la red desde el navegador).
#[wasm_bindgen]
pub fn quien(relay: &str) -> Result<String, JsError> {
    crate::relay::public_ip(relay).map_err(|e| JsError::new(&e.to_string()))
}

/// Pide el directorio firmado a un relé y lo comprueba.
#[wasm_bindgen]
pub fn directorio(relay: &str) -> Result<String, JsError> {
    let (d, _) = crate::directorio::descargar(&[relay.to_string()]).ok_or_else(|| JsError::new("el relé no dio un directorio válido"))?;
    serde_json::to_string(&d).map_err(|e| JsError::new(&e.to_string()))
}

// ---------- Mensajes ----------

/// Mi invitación (lo que va en el QR). Desde el navegador solo se llega por el relé.
#[wasm_bindgen]
pub fn invitacion() -> Result<String, JsError> {
    let (me, store) = abrir().map_err(js)?;
    let ruta = store.relay_route().map_err(js)?.ok_or_else(|| js("no hay ningún relé"))?;
    Ok(crate::invite(&me, &ruta))
}

/// Añade un contacto de confianza con su invitación.
#[wasm_bindgen]
pub fn anadir_contacto(nombre: &str, invitacion: &str) -> Result<(), JsError> {
    let (_, store) = abrir().map_err(js)?;
    let (key, ruta) = crate::parse_invite(invitacion).map_err(js)?;
    store.add_contact(nombre, &key, Some(&ruta)).map_err(js)
}

/// Envía un texto: «entregado», «en buzón» (el otro dormía) o «pendiente: …».
#[wasm_bindgen]
pub fn enviar(nombre: &str, texto: &str) -> Result<String, JsError> {
    let (me, store) = abrir().map_err(js)?;
    let (_, key, ruta) = store.contacts().map_err(js)?.into_iter().find(|c| c.0 == nombre).ok_or_else(|| js("contacto desconocido"))?;
    store.queue(&key, texto).map_err(js)?;
    Ok(match crate::send_or_deposit(&store, &me, &key, &ruta.unwrap_or_default()) {
        Ok(crate::Sent::Delivered(_)) => "entregado".into(),
        Ok(crate::Sent::Deposited(_)) => "en buzón".into(),
        Err(e) => format!("pendiente: {e}"),
    })
}

/// Todos los mensajes en JSON: `[{"contacto", "saliente", "texto", "estado"}]`.
#[wasm_bindgen]
pub fn mensajes() -> Result<String, JsError> {
    let (_, store) = abrir().map_err(js)?;
    let rows: Vec<serde_json::Value> = store
        .list()
        .map_err(js)?
        .into_iter()
        .map(|r| serde_json::json!({"contacto": r.peer, "saliente": r.outgoing, "texto": r.body, "estado": r.status}))
        .collect();
    serde_json::to_string(&rows).map_err(js)
}

/// Buzón en el relé, abierto entre una revisión y la siguiente (el navegador no tiene hilos).
struct Buzon {
    control: Stream,
    relay: String,
    ping: crate::tiempo::Instant,
    cartas: Option<crate::tiempo::Instant>,
    linea: Vec<u8>,
}

thread_local! {
    static BUZON: std::cell::RefCell<Option<Buzon>> = const { std::cell::RefCell::new(None) };
}

/// Revisa el buzón durante como mucho `ms`: atiende las llamadas que lleguen por el relé y, cada
/// minuto, recoge las cartas que esperaban. Devuelve cuántos mensajes nuevos llegaron. El SDK la
/// llama una y otra vez mientras la página está abierta.
#[wasm_bindgen]
pub fn revisar(ms: u32) -> Result<u32, JsError> {
    use crate::tiempo::Instant;
    use std::io::{Read, Write};
    let (me, store) = abrir().map_err(js)?;
    let mut b = match BUZON.take() {
        Some(b) => b,
        None => {
            let relay = store.relays().map_err(js)?.into_iter().next().ok_or_else(|| js("no hay ningún relé"))?;
            let control = crate::relay::register(&relay, &store.mailbox_secret().map_err(js)?).map_err(js)?;
            Buzon { control, relay, ping: Instant::now(), cartas: None, linea: Vec::new() }
        }
    };
    let mut nuevos = 0u32;
    if b.cartas.is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
        b.cartas = Some(Instant::now());
        nuevos += crate::collect_letters(&store, &me).unwrap_or(0) as u32;
    }
    // Un «PING» cada 25 s: si no, el relé cierra el buzón por silencio.
    if b.ping.elapsed() > Duration::from_secs(25) {
        b.ping = Instant::now();
        if writeln!(b.control, "PING").is_err() {
            return Ok(nuevos); // se volverá a registrar en la próxima revisión
        }
    }
    b.control.set_read_timeout(Some(Duration::from_millis(ms.max(1) as u64))).map_err(js)?;
    let mut byte = [0u8; 1];
    loop {
        match b.control.read(&mut byte) {
            Ok(0) => return Ok(nuevos), // el relé cerró: nuevo registro en la próxima revisión
            Ok(_) if byte[0] != b'\n' => b.linea.push(byte[0]),
            Ok(_) => {
                let linea = String::from_utf8_lossy(&std::mem::take(&mut b.linea)).into_owned();
                if let Some(token) = linea.strip_prefix("LLAMADA ") {
                    let llamada = crate::relay::answer(&b.relay, token).map_err(js)?;
                    nuevos += crate::serve(&store, &me, llamada).unwrap_or(0) as u32;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::TimedOut => break,
            Err(_) => return Ok(nuevos),
        }
    }
    BUZON.set(Some(b));
    Ok(nuevos)
}

// ---------- Colecciones compartidas ----------

/// Comparte una colección con un contacto: «leer», «escribir» o «nada» (quitar).
#[wasm_bindgen]
pub fn dar_permiso(coleccion: &str, contacto: &str, permiso: &str) -> Result<(), JsError> {
    let (_, store) = abrir().map_err(js)?;
    let key = store.contact_key(contacto).map_err(js)?.ok_or_else(|| js("contacto desconocido"))?;
    let acceso = match permiso {
        "leer" => Some(false),
        "escribir" => Some(true),
        "nada" => None,
        _ => return Err(js("permiso: leer | escribir | nada")),
    };
    store.set_share(coleccion, &key, acceso).map_err(js)
}

/// Sincroniza con un contacto lo que compartimos. Devuelve «N recibido(s), M enviado(s)…».
#[wasm_bindgen]
pub fn sincronizar(contacto: &str) -> Result<String, JsError> {
    let (me, store) = abrir().map_err(js)?;
    let (_, key, ruta) = store.contacts().map_err(js)?.into_iter().find(|c| c.0 == contacto).ok_or_else(|| js("contacto desconocido"))?;
    let (got, sent, rejected) = crate::sync_shared(&store, &me, &ruta.unwrap_or_default(), &key).map_err(js)?;
    Ok(format!("{got} recibido(s), {sent} enviado(s), {rejected} rechazado(s)"))
}
