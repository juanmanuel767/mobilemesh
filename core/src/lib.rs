//! Núcleo de MobileMesh: identidad, base de datos local y conexión directa cifrada.

pub mod access;
pub mod e2e;
pub mod files;
pub mod directorio;
pub mod funciones;
pub mod guardian;
pub mod identity;
pub mod net;
pub mod nombres;
pub mod publico;
pub mod recovery;
pub mod relay;
pub mod replica;
pub mod store;
pub mod sync;
#[cfg(not(target_arch = "wasm32"))]
pub mod supernodo;
pub mod transport;
pub mod upnp;
/// Reloj: el de std en móvil y PC; en el navegador, el del navegador (std no tiene reloj allí).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::time as tiempo;
#[cfg(target_arch = "wasm32")]
pub(crate) use web_time as tiempo;
#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(not(target_arch = "wasm32"))]
mod ffi;

#[cfg(not(target_arch = "wasm32"))]
uniffi::setup_scaffolding!();

pub use identity::{Identity, Pending};
pub use store::Store;

use identity::{Sealer, from_hex, from_hex_any, to_hex};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use files::FileMeta;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use transport::{Frame, Session};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Envía los mensajes pendientes para `peer` y devuelve cuántos confirmó el destino.
/// Lo que no se confirme sigue pendiente para el próximo intento.
pub fn deliver(store: &Store, me: &Identity, peer: &str, addr: &str) -> Result<usize> {
    let pending = store.pending(peer)?;
    if pending.is_empty() {
        return Ok(0);
    }
    if store.is_blocked(peer)? {
        return Err("contacto bloqueado".into());
    }
    let mut s = connect(store, me, addr, &from_hex(peer)?)?;
    let device = to_hex(&s.device);
    store.set_contact_device(peer, &device)?;
    ensure_session(&mut s, store, &device)?;
    let mut sent = 0;
    for (id, body, file) in &pending {
        let (kind, ct) = e2e::encrypt(store, &device, id, body, file.as_ref())?;
        s.send(&Frame::Msg { id: id.clone(), kind, body: ct })?;
        let mut reply = s.recv()?;
        if let (Some(meta), Some(Frame::Have { chunks, .. })) = (file, &reply) {
            send_chunks(&mut s, store, peer, id, meta, *chunks)?;
            // El destino comprueba la huella del archivo entero antes de confirmar: con archivos grandes
            // o teléfonos lentos tarda. Se espera 10 s más 1 s por MB.
            s.set_timeout(std::time::Duration::from_secs(10 + meta.size / 1_000_000))?;
            reply = s.recv()?;
            s.set_timeout(std::time::Duration::ZERO)?;
        }
        match reply {
            Some(Frame::Ack { id: a }) if a == *id => {
                store.mark_delivered(peer, id)?;
                sent += 1;
            }
            // Límite de mensajes del destino: no es un error del mensaje, queda pendiente.
            Some(Frame::Bad { reason, .. }) if reason.starts_with(access::LIMIT) => return Err(reason.into()),
            Some(Frame::Bad { reason, .. }) => {
                store.set_status(peer, true, id, "error")?;
                return Err(format!("el destino rechazó el archivo: {reason}").into());
            }
            Some(Frame::NoSession { .. }) => {
                e2e::forget(store, &device)?;
                return Err("el destino perdió la sesión cifrada; se rehará en el próximo intento".into());
            }
            Some(_) => return Err("respuesta inesperada del destino".into()),
            None => return Err("el destino cerró la conexión sin confirmar".into()),
        }
    }
    Ok(sent)
}

/// Conecta con la cuenta `expect` y hace el intercambio de listas de retirados (Fase 10).
pub fn connect(store: &Store, me: &Identity, route: &str, expect: &[u8]) -> Result<Session> {
    let mut s = Session::connect(route, me, expect)?;
    access::exchange(&mut s, store, me)?;
    Ok(s)
}

/// Abre la sesión del ratchet con el dispositivo del otro lado si todavía no hay una.
fn ensure_session(s: &mut Session, store: &Store, device: &str) -> Result<()> {
    if !e2e::has_session(store, device)? {
        s.send(&Frame::KeysReq)?;
        let Some(Frame::Keys { identity, one_time }) = s.recv()? else {
            return Err("el destino no envió sus claves de cifrado".into());
        };
        e2e::start_session(store, device, &identity, &one_time)?;
    }
    Ok(())
}

/// Envía los trozos de un archivo a partir de `from` (los anteriores ya los tiene el destino).
fn send_chunks(s: &mut Session, store: &Store, peer: &str, id: &str, meta: &FileMeta, from: u64) -> Result<()> {
    let blob = store.blob(peer, true, id)?;
    let total = meta.chunks();
    for index in from..total {
        let data = B64.encode(files::read_chunk(&blob, meta, index)?);
        s.send(&Frame::Chunk { id: id.into(), index, data })?;
        if index % 4 == 3 || index + 1 == total {
            store.set_progress(peer, true, id, index + 1, total)?;
        }
    }
    Ok(())
}

/// Archivo que está llegando: (identificador, datos, recepción a medias).
type Incoming = (String, FileMeta, PathBuf);

/// Identificador de mensaje válido: 32 caracteres hexadecimales en minúscula.
fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Conexiones atendidas a la vez como máximo. Cada una va en su hilo: una conexión lenta o
/// maliciosa no puede bloquear a las demás (encontrado en los ataques simulados de la Fase 13).
pub const MAX_CONNECTIONS: usize = 16;

/// Atiende conexiones entrantes para siempre, cada una en su propio hilo. `pause()` = no atender
/// (modo sin conexión). Limita los intentos por IP antes del saludo. `done` recibe el
/// resultado de cada conexión.
pub fn accept_loop(
    listener: std::net::TcpListener,
    dir: PathBuf,
    me: Identity,
    pause: impl Fn() -> bool,
    done: impl Fn(Result<usize>) + Send + Sync + 'static,
) {
    let done = std::sync::Arc::new(done);
    for stream in listener.incoming().flatten() {
        let _ = stream.set_nodelay(true);
        if pause() {
            continue; // se cierra sin contestar
        }
        // Límite de intentos por dirección, antes de gastar nada en el saludo.
        let ip = stream.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
        if !access::allow(&format!("ip:{ip}"), access::CONNECTIONS_PER_MINUTE, std::time::Duration::from_secs(60)) {
            continue;
        }
        let Some(slot) = Slot::take(&ip) else { continue };
        let (done, dir, me) = (done.clone(), dir.clone(), me.clone());
        std::thread::spawn(move || {
            done(Store::open(&dir, me.db_key()).and_then(|store| serve(&store, &me, stream)));
            drop(slot);
        });
    }
}

/// Conexiones a la vez como máximo desde una misma dirección (las del relé llegan todas de la suya).
pub const MAX_PER_IP: usize = 4;

static ACTIVE: std::sync::Mutex<Option<std::collections::HashMap<String, usize>>> = std::sync::Mutex::new(None);

/// Hueco para atender una conexión: lo comparten la escucha directa y las llamadas por el relé.
/// Se libera solo al terminar.
pub struct Slot(String);

impl Slot {
    /// `None` si ya hay demasiadas conexiones en total o desde `origin`.
    pub fn take(origin: &str) -> Option<Slot> {
        let mut g = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        let map = g.get_or_insert_with(Default::default);
        let total: usize = map.values().sum();
        let mine = map.get(origin).copied().unwrap_or(0);
        // Las llamadas por el relé tienen su propio cupo: el buzón es público y nadie debe poder
        // ocupar por ahí todos los huecos (verificación de la auditoría, Fase 13).
        let limit = if origin.starts_with("rele:") { MAX_CONNECTIONS / 2 } else { MAX_PER_IP };
        if total >= MAX_CONNECTIONS || mine >= limit {
            return None;
        }
        map.insert(origin.into(), mine + 1);
        Some(Slot(origin.into()))
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut g = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = g.as_mut()
            && let Some(n) = map.get_mut(&self.0)
        {
            *n -= 1;
            if *n == 0 {
                map.remove(&self.0);
            }
        }
    }
}

/// Atiende una conexión entrante; solo acepta contactos conocidos. Devuelve cuántos mensajes recibió.
pub fn serve(store: &Store, me: &Identity, stream: impl net::Conn + 'static) -> Result<usize> {
    let mut s = Session::accept(stream, me)?;
    let peer = to_hex(&s.peer);
    // Se aceptan los contactos y los otros dispositivos de mi propia cuenta. Un ayudante «abierto»
    // (funciones de servidor) acepta a cualquiera y lo apunta como contacto.
    if peer != me.id() && store.contact_name(&peer)?.is_none() {
        if store.setting("abierto")?.as_deref() != Some("1") {
            return Err(format!("clave desconocida {}…", &peer[..12]).into());
        }
        store.add_contact(&format!("usuario-{}", &peer[..12]), &peer, None)?;
    }
    access::exchange(&mut s, store, me)?;
    let device = to_hex(&s.device);
    let mut n = 0;
    let mut incoming: Option<Incoming> = None;
    // Archivo rechazado: sus trozos que aún vengan de camino se descartan sin cortar la conexión,
    // para que el remitente llegue a leer el rechazo.
    let mut rejected: Option<String> = None;
    let mut replica_in: Option<replica::Incoming> = None;
    // Un guardián acepta a cualquiera, pero solo para copias y códigos de cuenta: mensajes y
    // archivos de desconocidos llenarían su disco (auditoría de las piezas nuevas).
    let solo_guardian = guardian::is_guardian(store)?;
    let solo_nombres = nombres::is_guide(store)?;
    let solo_publico = publico::is_public(store)?;
    while let Some(frame) = s.recv()? {
        if solo_guardian && !guardian::allowed(&frame) {
            return Err("un guardián solo guarda copias y códigos de cuenta".into());
        }
        if solo_nombres && !nombres::allowed(&frame) {
            return Err("una guía de nombres solo atiende nombres".into());
        }
        if solo_publico && !publico::allowed(&frame) {
            return Err("una pieza pública solo atiende colecciones públicas".into());
        }
        match frame {
            Frame::KeysReq => {
                if !access::allow(&format!("llaves:{peer}"), 20, std::time::Duration::from_secs(60)) {
                    return Err("demasiadas peticiones de claves".into());
                }
                let (identity, one_time) = e2e::publish_keys(store)?;
                s.send(&Frame::Keys { identity, one_time })?;
            }
            // El identificador lo elige el remitente y acaba en nombres de archivo: solo 32 hex.
            Frame::Msg { id, .. } if !valid_id(&id) => return Err("identificador de mensaje no válido".into()),
            Frame::Msg { id, .. } if !access::allow(&peer, access::MESSAGES_PER_MINUTE, std::time::Duration::from_secs(60)) => {
                s.send(&Frame::Bad { id, reason: format!("{}: demasiados mensajes seguidos; espera un minuto", access::LIMIT) })?;
                return Err("demasiados mensajes de ese contacto".into());
            }
            Frame::Msg { id, kind, body } => match e2e::decrypt(store, &device, &id, kind, &body)? {
                Some((text, None)) => {
                    store.save_incoming(&peer, &id, &text)?;
                    s.send(&Frame::Ack { id })?; // solo se confirma después de guardar
                    n += usize::from(!funciones::is_control(&text)); // las respuestas no avisan

                }
                Some((_, Some(meta))) if meta.size > files::MAX_SIZE => {
                    s.send(&Frame::Bad { id, reason: "archivo demasiado grande".into() })?;
                }
                Some((_, Some(meta))) => {
                    if store.incoming_file(&peer, &id, &meta)? == "guardado" {
                        s.send(&Frame::Ack { id })?; // ya lo teníamos completo
                        continue;
                    }
                    let part = store.blob(&peer, false, &id)?.with_extension("parcial");
                    let have = files::received_chunks(&part)?;
                    s.send(&Frame::Have { id: id.clone(), chunks: have })?;
                    incoming = Some((id, meta, part));
                    if have == incoming.as_ref().map_or(0, |f| f.1.chunks()) {
                        n += finish_file(&mut s, store, &peer, incoming.take())?;
                    }
                }
                None => s.send(&Frame::NoSession { id })?,
            },
            Frame::Sync { .. } | Frame::ReplicaPut { .. } | Frame::ReplicaCheck { .. } | Frame::ReplicaGet | Frame::ReplicaDel
                if !access::allow(&format!("pesado:{peer}"), 20, std::time::Duration::from_secs(60)) =>
            {
                return Err("demasiadas sincronizaciones o copias seguidas".into());
            }
            Frame::Sync { id, kind, body } => {
                let Some((text, None)) = e2e::decrypt(store, &device, &id, kind, &body)? else {
                    s.send(&Frame::NoSession { id })?;
                    continue;
                };
                let SyncMsg::Hello { vector, collections } = serde_json::from_str(&text)? else {
                    return Err("la sincronización debe empezar con un saludo".into());
                };
                let scope = match collections {
                    None if peer == me.id() => Scope::own(),
                    None => return Err("solo los dispositivos de la misma cuenta pueden sincronizarlo todo".into()),
                    // Un contacto: solo lo que le he compartido, y solo acepto cambios suyos donde puede escribir.
                    Some(wanted) => Scope::shared(store, &peer, Some(&wanted))?,
                };
                let scope = Scope { device: device.clone(), ..scope };
                n += answer_sync(&mut s, store, &device, &scope, vector)?;
            }
            Frame::ReplicaPut { version, meta, size } => replica_in = replica::on_put(&mut s, store, me, &peer, version, meta, size)?,
            Frame::Chunk { id, index, data } if id == replica::CHUNK_ID => {
                let r = replica_in.take().ok_or("trozo de copia inesperado")?;
                replica_in = replica::on_chunk(&mut s, store, r, index, &data)?;
            }
            Frame::ReplicaCheck { nonce } => replica::on_check(&mut s, store, &peer, &nonce)?,
            Frame::ReplicaGet => replica::on_get(&mut s, store, &peer)?,
            Frame::ReplicaDel => replica::on_delete(&mut s, store, &peer)?,
            Frame::CuentaPut { id, code } => guardian::on_put(&mut s, store, &peer, &id, &code)?,
            Frame::CuentaGet { id } => guardian::on_get(&mut s, store, &peer, &id)?,
            Frame::NombrePut { app, nombre, invitacion } => nombres::on_put(&mut s, store, &peer, &app, &nombre, &invitacion)?,
            Frame::NombreGet { app, nombre } => nombres::on_get(&mut s, store, &peer, &app, &nombre)?,
            Frame::NombreSolicitar { app, nombre } => nombres::on_request(&mut s, store, &peer, &app, &nombre)?,
            Frame::NombreSolicitudes => nombres::on_requests(&mut s, store, &peer)?,
            Frame::PublicoPut { app, coleccion, id, campos } => publico::on_put(&mut s, store, &peer, &app, &coleccion, &id, &campos)?,
            Frame::PublicoBorrar { app, coleccion, id } => publico::on_delete(&mut s, store, &peer, &app, &coleccion, &id)?,
            Frame::PublicoConsulta { app, coleccion, filtros, orden, desc, limite } => {
                publico::on_query(&mut s, store, &peer, &app, &coleccion, &filtros, orden.as_deref(), desc, limite)?
            }
            Frame::Chunk { id, .. } if rejected.as_ref() == Some(&id) => {}
            Frame::Chunk { id, index, data } => {
                let Some((fid, meta, part)) = &incoming else { return Err("trozo de archivo inesperado".into()) };
                if *fid != id {
                    return Err("trozo de otro archivo".into());
                }
                let total = meta.chunks();
                let data = B64.decode(&data).map_err(|_| "trozo mal codificado".into());
                if let Err(e) = data.and_then(|d| files::append_chunk(part, meta, index, &d)) {
                    // No se acepta un archivo con un trozo que no supera la comprobación.
                    let _ = std::fs::remove_file(part);
                    s.send(&Frame::Bad { id: id.clone(), reason: e.to_string() })?;
                    rejected = Some(id);
                    incoming = None;
                    continue;
                }
                if index % 4 == 3 || index + 1 == total {
                    store.set_progress(&peer, false, &id, index + 1, total)?;
                }
                if index + 1 == total {
                    n += finish_file(&mut s, store, &peer, incoming.take())?;
                }
            }
            _ => return Err("paquete inesperado".into()),
        }
    }
    Ok(n)
}

/// Comprueba la huella del archivo completo; si es correcta lo guarda y lo confirma.
fn finish_file(s: &mut Session, store: &Store, peer: &str, incoming: Option<Incoming>) -> Result<usize> {
    let Some((id, meta, part)) = incoming else { return Ok(0) };
    match files::decrypt_file(&part, &meta, None) {
        Ok(()) => {
            std::fs::rename(&part, store.blob(peer, false, &id)?)?;
            store.set_status(peer, false, &id, "guardado")?;
            s.send(&Frame::Ack { id })?;
            Ok(1)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            s.send(&Frame::Bad { id, reason: e.to_string() })?;
            Ok(0)
        }
    }
}

// ---------- Sincronización entre los dispositivos de una cuenta (Fase 7) ----------

/// Lo que va (cifrado con el ratchet) en los paquetes de sincronización.
#[derive(Serialize, Deserialize)]
enum SyncMsg {
    /// «Esto es lo que tengo»: el que empieza manda su vector de versiones.
    /// Con un contacto, `collections` dice qué colecciones compartidas quiere sincronizar.
    Hello {
        vector: sync::Vector,
        #[serde(default)]
        collections: Option<Vec<String>>,
    },
    /// Operaciones que le faltan al otro, por tandas; la última lleva `more: false`.
    Data { ops: Vec<sync::Op>, more: bool, vector: sync::Vector },
    /// Fin: el que contesta dice qué tiene ya, para saber qué está confirmado.
    Done { vector: sync::Vector },
}

/// Tamaño aproximado de cada tanda (cabe de sobra en un paquete Noise tras cifrar).
const SYNC_BATCH: usize = 20_000;

fn send_sync(s: &mut Session, store: &Store, device: &str, msg: &SyncMsg) -> Result<()> {
    let id = store.new_id()?;
    let (kind, body) = e2e::encrypt(store, device, &id, &serde_json::to_string(msg)?, None)?;
    s.send(&Frame::Sync { id, kind, body })
}

fn recv_sync(s: &mut Session, store: &Store, device: &str) -> Result<SyncMsg> {
    match s.recv()? {
        Some(Frame::Sync { id, kind, body }) => match e2e::decrypt(store, device, &id, kind, &body)? {
            Some((text, None)) => Ok(serde_json::from_str(&text)?),
            _ => Err("paquete de sincronización no válido".into()),
        },
        Some(Frame::NoSession { .. }) => {
            e2e::forget(store, device)?;
            Err("el otro dispositivo perdió la sesión cifrada; se rehará en el próximo intento".into())
        }
        Some(_) => Err("respuesta inesperada durante la sincronización".into()),
        None => Err("el otro dispositivo cerró la conexión".into()),
    }
}

/// Qué se sincroniza con el otro lado: todo (mis dispositivos) o solo lo compartido (un contacto).
struct Scope {
    /// `None` = mis dispositivos: todo y sin filtrar.
    contact: Option<String>,
    cols: Vec<String>,
    writable: Vec<String>,
    /// Dispositivo del otro lado en esta conexión (comprobado en el saludo).
    device: String,
}

impl Scope {
    fn own() -> Self {
        Scope { contact: None, cols: vec![], writable: vec![], device: String::new() }
    }

    /// Colecciones que comparto con `peer` (y, si se indica, que él pide).
    fn shared(store: &Store, peer: &str, wanted: Option<&[String]>) -> Result<Self> {
        let shares = store.shares(peer)?;
        let cols = shares.iter().map(|s| s.0.clone()).filter(|c| wanted.is_none_or(|w| w.contains(c))).collect();
        let writable = shares.into_iter().filter(|s| s.1).map(|s| s.0).collect();
        Ok(Scope { contact: Some(peer.into()), cols, writable, device: String::new() })
    }

    fn vector(&self, store: &Store) -> Result<sync::Vector> {
        match self.contact {
            None => store.vector(),
            Some(_) => store.vector_in(&self.cols),
        }
    }

    fn ops_after(&self, store: &Store, have: &sync::Vector) -> Result<Vec<sync::Op>> {
        match self.contact {
            None => store.ops_after(have),
            Some(_) => store.ops_after_in(have, &self.cols),
        }
    }

    /// ¿Se acepta este cambio que llega del otro lado? De un contacto: solo donde puede escribir, a su
    /// nombre, con un dispositivo que sea suyo y sin cerrar conflictos de otros.
    fn accepts(&self, store: &Store, op: &sync::Op) -> Result<bool> {
        let Some(peer) = &self.contact else { return Ok(true) };
        if !self.writable.contains(&op.collection) || op.author != *peer {
            return Ok(false);
        }
        match store.device_author(&op.device)? {
            Some(a) if a != *peer => return Ok(false),
            // Dispositivo que aún no conozco: solo lo acepto si es el que está hablando conmigo,
            // cuya pertenencia a la cuenta ya está comprobada; así nadie puede reclamar el
            // dispositivo de otra persona (verificación de la auditoría, Fase 13).
            None if op.device != self.device => return Ok(false),
            _ => {}
        }
        for (device, _) in &op.resolves {
            if store.device_author(device)?.is_some_and(|a| a != *peer) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Manda operaciones en tandas; la última lleva el vector de versiones.
fn send_ops(s: &mut Session, store: &Store, device: &str, ops: Vec<sync::Op>, vector: sync::Vector) -> Result<usize> {
    let total = ops.len();
    let mut batch = Vec::new();
    let mut size = 0;
    for op in ops {
        let n = serde_json::to_string(&op)?.len();
        if !batch.is_empty() && size + n > SYNC_BATCH {
            send_sync(s, store, device, &SyncMsg::Data { ops: std::mem::take(&mut batch), more: true, vector: Default::default() })?;
            size = 0;
        }
        size += n;
        batch.push(op);
    }
    send_sync(s, store, device, &SyncMsg::Data { ops: batch, more: false, vector })?;
    Ok(total)
}

/// Cambios como máximo en una sincronización (el resto llegará en la siguiente).
const MAX_SYNC_OPS: usize = 100_000;

/// Recibe tandas hasta la última y las incorpora si están permitidas.
/// Devuelve (nuevas, rechazadas, vector del otro).
fn recv_ops(s: &mut Session, store: &Store, device: &str, scope: &Scope) -> Result<(usize, usize, sync::Vector)> {
    let (mut new, mut rejected) = (0, 0);
    loop {
        let SyncMsg::Data { ops, more, vector } = recv_sync(s, store, device)? else {
            return Err("se esperaban datos de sincronización".into());
        };
        if new + rejected + ops.len() > MAX_SYNC_OPS {
            return Err("demasiados cambios en una sola sincronización".into());
        }
        let mut ok = Vec::new();
        for op in ops {
            if scope.accepts(store, &op)? {
                ok.push(op);
            } else {
                rejected += 1;
            }
        }
        new += sync::merge(store, ok)?;
        if !more {
            return Ok((new, rejected, vector));
        }
    }
}

/// Sincroniza con otro dispositivo de mi cuenta. Devuelve (cambios recibidos, cambios enviados).
pub fn sync_with(store: &Store, me: &Identity, addr: &str) -> Result<(usize, usize)> {
    let (got, sent, _) = sync_scoped(store, me, addr, &me.account, Scope::own())?;
    Ok((got, sent))
}

/// Sincroniza con un contacto las colecciones compartidas con él.
/// Devuelve (cambios recibidos, enviados, rechazados por falta de permiso).
pub fn sync_shared(store: &Store, me: &Identity, addr: &str, peer: &str) -> Result<(usize, usize, usize)> {
    let scope = Scope::shared(store, peer, None)?;
    if scope.cols.is_empty() {
        return Err("no compartes ninguna colección con ese contacto".into());
    }
    sync_scoped(store, me, addr, &from_hex(peer)?, scope)
}

fn sync_scoped(store: &Store, me: &Identity, addr: &str, expect: &[u8], scope: Scope) -> Result<(usize, usize, usize)> {
    let mut s = connect(store, me, addr, expect)?;
    let device = to_hex(&s.device);
    let scope = Scope { device: device.clone(), ..scope };
    ensure_session(&mut s, store, &device)?;
    let collections = scope.contact.as_ref().map(|_| scope.cols.clone());
    send_sync(&mut s, store, &device, &SyncMsg::Hello { vector: scope.vector(store)?, collections })?;
    let (got, rejected, theirs) = recv_ops(&mut s, store, &device, &scope)?;
    let sent = send_ops(&mut s, store, &device, scope.ops_after(store, &theirs)?, scope.vector(store)?)?;
    let SyncMsg::Done { vector } = recv_sync(&mut s, store, &device)? else {
        return Err("el otro lado no terminó la sincronización".into());
    };
    if scope.contact.is_none() {
        store.set_peer_vector(&device, &vector)?;
    }
    Ok((got, sent, rejected))
}

/// Lado que contesta: manda lo que le falta al otro, recibe lo suyo y confirma.
fn answer_sync(s: &mut Session, store: &Store, device: &str, scope: &Scope, theirs: sync::Vector) -> Result<usize> {
    send_ops(s, store, device, scope.ops_after(store, &theirs)?, scope.vector(store)?)?;
    let (got, _, _) = recv_ops(s, store, device, scope)?;
    let mine = scope.vector(store)?;
    send_sync(s, store, device, &SyncMsg::Done { vector: mine.clone() })?;
    // Tras esto el otro dispositivo propio tiene todo lo mío: lo apunto como confirmado.
    if scope.contact.is_none() {
        store.set_peer_vector(device, &mine)?;
    }
    Ok(got)
}

/// Añade un contacto; si la invitación es de mi propia cuenta, es otro dispositivo mío (para sincronizar).
pub fn add_contact_or_device(store: &Store, me: &Identity, name: &str, key: &str, addr: Option<&str>) -> Result<bool> {
    if key == me.id() {
        store.add_my_device(name, addr.ok_or("hace falta la dirección del otro dispositivo")?)?;
        return Ok(true);
    }
    store.add_contact(name, key, addr)?;
    Ok(false)
}

/// Cifra un archivo para `peer` y lo deja pendiente de envío. El original no se toca.
pub fn queue_file(store: &Store, peer: &str, src: &Path, name: &str, mime: &str) -> Result<String> {
    let id = store.new_id()?;
    let blob = store.blob(peer, true, &id)?;
    let meta = files::encrypt_file(src, &blob, name, mime).inspect_err(|_| {
        let _ = std::fs::remove_file(&blob);
    })?;
    store.queue_file(peer, &id, &meta)?;
    Ok(id)
}

/// Descifra el archivo de un mensaje en `dst` (para verlo o guardarlo fuera de la app).
pub fn export_file(store: &Store, peer: &str, outgoing: bool, id: &str, dst: &Path) -> Result<()> {
    let meta = store.file_of(peer, outgoing, id)?.ok_or("ese mensaje no tiene archivo")?;
    let mut out = std::fs::File::create(dst)?;
    files::decrypt_file(&store.blob(peer, outgoing, id)?, &meta, Some(&mut out)).inspect_err(|_| {
        let _ = std::fs::remove_file(dst);
    })
}

const INVITE: &str = "mobilemesh:2:";

/// Invitación para que otro dispositivo nos añada (va en el QR). Solo lleva datos públicos.
pub fn invite(me: &Identity, addr: &str) -> String {
    format!("{INVITE}{}:{addr}", me.id())
}

/// Lee una invitación y devuelve (clave, dirección).
pub fn parse_invite(s: &str) -> Result<(String, String)> {
    let s = s.trim();
    if s.starts_with("mobilemesh:1:") {
        return Err("esa invitación es de una versión anterior; pide una nueva".into());
    }
    let rest = s.strip_prefix(INVITE).ok_or("no es una invitación de MobileMesh")?;
    let (key, addr) = rest.split_once(':').ok_or("invitación incompleta")?;
    from_hex(key)?;
    transport::check_route(addr)?;
    Ok((key.to_string(), addr.to_string()))
}

const LINK_REQUEST: &str = "mobilemesh-vincular:1:";
const LINK_CERT: &str = "mobilemesh-certificado:1:";

/// Solicitud que muestra un dispositivo nuevo (en QR) para que el principal de la cuenta lo vincule.
pub fn link_request(p: &Pending) -> String {
    format!("{LINK_REQUEST}{}", to_hex(&p.public))
}

/// Código corto para comprobar a simple vista que los dos teléfonos hablan del mismo dispositivo.
pub fn fingerprint(device_public: &[u8]) -> String {
    let h = to_hex(&device_public[..6]).to_uppercase();
    format!("{}-{}-{}", &h[..4], &h[4..8], &h[8..])
}

/// Clave del dispositivo que pide vincularse.
pub fn parse_link_request(s: &str) -> Result<Vec<u8>> {
    from_hex(s.trim().strip_prefix(LINK_REQUEST).ok_or("no es una solicitud de vinculación de MobileMesh")?)
}

/// El dispositivo principal aprueba la solicitud: devuelve el certificado (público) para el nuevo.
pub fn approve_link(me: &Identity, request: &str) -> Result<String> {
    let cert = me.approve(&parse_link_request(request)?)?;
    Ok(format!("{LINK_CERT}{}:{}", me.id(), to_hex(&cert)))
}

/// El dispositivo nuevo instala el certificado tras comprobar que es para él.
pub fn complete_link(p: Pending, dir: &std::path::Path, s: &dyn Sealer, answer: &str) -> Result<Identity> {
    let rest = answer.trim().strip_prefix(LINK_CERT).ok_or("no es un certificado de vinculación de MobileMesh")?;
    let (account, cert) = rest.split_once(':').ok_or("certificado incompleto")?;
    p.complete(dir, s, &from_hex(account)?, &from_hex_any(cert)?)
}

// ---------- Buzón de cartas: mensajes para quien está dormido ----------

/// Carta del buzón (va en base64). El contenido va cifrado con el ratchet de extremo a extremo.
#[derive(Serialize, Deserialize)]
struct Letter {
    /// "msg" (mensaje) o "ack" (acuse de recibo).
    t: String,
    /// Dispositivo que la escribe.
    de: String,
    id: String,
    kind: usize,
    body: String,
    /// Buzón de quien la escribe («host:puerto/buzón»), para devolverle el acuse aunque no lo tenga
    /// apuntado. Va sin cifrar: quien lo cambie solo desvía un acuse cifrado que no puede leer.
    #[serde(default)]
    buzon: Option<String>,
}

/// Partes «relay=host:puerto/buzón» de una ruta: (relé, buzón).
fn relay_parts(route: &str) -> impl Iterator<Item = (String, String)> + '_ {
    route.split('|').filter_map(|p| p.trim().strip_prefix("relay=")?.rsplit_once('/').map(|(r, b)| (r.into(), b.into())))
}

fn relay_part(route: &str) -> Option<(String, String)> {
    relay_parts(route).next()
}

fn letter(store: &Store, t: &str, me: &Identity, id: &str, kind: usize, body: String) -> Result<String> {
    let buzon = store.relay_route()?.and_then(|r| r.split('|').next()?.strip_prefix("relay=").map(String::from));
    let l = Letter { t: t.into(), de: to_hex(&me.public), id: id.into(), kind, body, buzon };
    Ok(B64.encode(serde_json::to_vec(&l)?))
}

/// Resultado de un envío: entregado en directo o dejado en el buzón del relé.
#[derive(Debug, PartialEq)]
pub enum Sent {
    Delivered(usize),
    Deposited(usize),
}

/// Entrega en directo; si el destino no contesta (dormido, sin red) y tiene buzón en un relé, deja
/// allí los mensajes de texto como cartas cifradas. Lo demás sigue pendiente.
pub fn send_or_deposit(store: &Store, me: &Identity, peer: &str, route: &str) -> Result<Sent> {
    match deliver(store, me, peer, route) {
        Ok(n) => Ok(Sent::Delivered(n)),
        Err(e) => match deposit_pending(store, me, peer, route)? {
            0 => Err(e),
            n => Ok(Sent::Deposited(n)),
        },
    }
}

/// Deja los mensajes de texto pendientes en el buzón del destinatario. Solo si ya hablamos alguna
/// vez (hace falta la sesión del ratchet con su dispositivo).
// ponytail: solo texto; los archivos esperan a la entrega directa.
pub fn deposit_pending(store: &Store, me: &Identity, peer: &str, route: &str) -> Result<usize> {
    let boxes: Vec<_> = relay_parts(route).collect();
    let (false, Some(device)) = (boxes.is_empty(), store.contact_device(peer)?) else { return Ok(0) };
    if !e2e::has_session(store, &device)? {
        return Ok(0);
    }
    let mut n = 0;
    for (id, body, file) in store.pending(peer)? {
        if file.is_some() {
            continue;
        }
        let (kind, ct) = e2e::encrypt(store, &device, &id, &body, None)?;
        let carta = letter(store, "msg", me, &id, kind, ct)?;
        // El primer relé que la acepte; si ninguno, el error del último.
        let mut r = Err("sin relé".into());
        for (relay, mailbox) in &boxes {
            r = relay::deposit(relay, mailbox, &carta);
            if r.is_ok() {
                break;
            }
        }
        r?;
        store.set_status(peer, true, &id, "en buzón")?;
        n += 1;
    }
    Ok(n)
}

/// Recoge las cartas de mi buzón: guarda los mensajes, deja el acuse de recibo en el buzón de cada
/// remitente y marca como entregados los míos que ya confirmaron. Devuelve los mensajes nuevos.
pub fn collect_letters(store: &Store, me: &Identity) -> Result<usize> {
    let relays = store.relays()?;
    if relays.is_empty() {
        return Ok(0);
    }
    let secret = store.mailbox_secret()?;
    store.requeue_stale_letters(6)?;
    let mut new = 0;
    let mut acks: Vec<Ack> = Vec::new();
    // De cada relé; falla solo si no responde ninguno.
    let mut ok = false;
    let mut last = Ok(0);
    for relay in &relays {
        last = relay::collect(relay, &secret, |raw| {
            if let Ok(Some(ack)) = read_letter(store, me, raw, &mut new) {
                acks.push(ack);
            }
            true // se borra siempre: lo ilegible o ajeno no se guarda
        });
        ok |= last.is_ok();
    }
    if !ok {
        last?;
    }
    for (account, device, id, buzon) in acks {
        let route = store.contacts()?.into_iter().find(|c| c.1 == account).and_then(|c| c.2);
        let target = route.as_deref().and_then(relay_part).or_else(|| buzon.and_then(|b| b.rsplit_once('/').map(|(r, m)| (r.into(), m.into()))));
        if let Some((relay, mailbox)) = target {
            let (kind, ct) = e2e::encrypt(store, &device, &id, "ack", None)?;
            let _ = relay::deposit(&relay, &mailbox, &letter(store, "ack", me, &id, kind, ct)?);
        }
    }
    Ok(new)
}

/// Acuse pendiente: (cuenta, dispositivo, id del mensaje, buzón que trae la carta).
type Ack = (String, String, String, Option<String>);

/// Lee una carta. Devuelve a quién hay que mandar acuse de recibo (si era un mensaje nuevo).
fn read_letter(store: &Store, me: &Identity, raw: &str, new: &mut usize) -> Result<Option<Ack>> {
    let l: Letter = serde_json::from_slice(&B64.decode(raw).map_err(|_| "carta mal codificada")?)?;
    // Solo de dispositivos cuya cuenta ya comprobé en un saludo, y de contactos no bloqueados.
    let account = store.device_account(&l.de)?.ok_or("carta de un dispositivo desconocido")?;
    if (account != me.id() && store.contact_name(&account)?.is_none()) || store.is_blocked(&account)? || store.is_revoked(&account, &l.de)? {
        return Err("carta no permitida".into());
    }
    if !valid_id(&l.id) {
        return Err("identificador no válido".into());
    }
    let Some((text, None)) = e2e::decrypt(store, &l.de, &l.id, l.kind, &l.body)? else { return Ok(None) };
    match l.t.as_str() {
        "msg" => {
            store.save_incoming(&account, &l.id, &text)?;
            *new += usize::from(!funciones::is_control(&text));
            Ok(Some((account, l.de, l.id, l.buzon)))
        }
        "ack" => {
            store.mark_delivered(&account, &l.id)?;
            Ok(None)
        }
        _ => Ok(None),
    }
}
