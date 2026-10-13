//! Copias de seguridad cifradas guardadas en otros dispositivos (Fase 8).
//!
//! - La copia (contactos, mensajes y datos sincronizados) se cifra como un archivo y su clave se
//!   envuelve con una clave sacada de la **clave de la cuenta**: quien la guarda no puede leerla, y
//!   quien recupera la cuenta con su copia de recuperación (`MM2-`) sí puede abrirla.
//! - Guardar copias de otros es **voluntario**: hay que aceptarlo para cada contacto. Los
//!   dispositivos propios las aceptan siempre.
//! - Para comprobar que alguien sigue teniendo la copia intacta no hace falta descargarla: se le
//!   pide la huella de (número al azar + copia), que no puede calcular sin tenerla entera.
//! - Borrar: se pide a cada uno que la borre; si no está conectado, se reintenta cuando vuelva.

use crate::{
    Identity, Result, Store,
    files::{self, FileMeta},
    identity::{from_hex, to_hex},
    store::{Holder, SnapMsg},
    sync,
    transport::{Frame, Session},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, Payload},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::PathBuf, time::Duration};

/// Identificador de los trozos de copia en el protocolo (los de archivos usan el id del mensaje).
pub const CHUNK_ID: &str = "copia";

#[derive(Serialize, Deserialize)]
struct Snapshot {
    contacts: Vec<(String, String, Option<String>)>,
    devices: Vec<(String, String)>,
    messages: Vec<SnapMsg>,
    ops: Vec<sync::Op>,
}

fn backup_key(me: &Identity) -> Result<[u8; 32]> {
    let secret = me.account_secret.ok_or("las copias se hacen desde el dispositivo principal de la cuenta")?;
    Ok(Sha256::new_with_prefix(b"mobilemesh-copias-v1").chain_update(secret).finalize().into())
}

/// La cuenta y la versión van autenticadas: nadie puede hacer pasar una copia vieja por nueva.
fn aad(me: &Identity, version: u64) -> Vec<u8> {
    [me.account.as_slice(), &version.to_be_bytes()].concat()
}

fn wrap(me: &Identity, version: u64, meta: &FileMeta) -> Result<String> {
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let ct = ChaCha20Poly1305::new(&backup_key(me)?.into())
        .encrypt(&nonce.into(), Payload { msg: &serde_json::to_vec(meta)?, aad: &aad(me, version) })
        .map_err(|_| "no se pudo cifrar la copia")?;
    Ok(B64.encode([nonce.as_slice(), &ct].concat()))
}

fn unwrap(me: &Identity, version: u64, wrapped: &str) -> Result<FileMeta> {
    let b = B64.decode(wrapped).map_err(|_| "copia mal codificada")?;
    let (nonce, ct) = b.split_at_checked(12).ok_or("copia incompleta")?;
    let plain = ChaCha20Poly1305::new(&backup_key(me)?.into())
        .decrypt(nonce.into(), Payload { msg: ct, aad: &aad(me, version) })
        .map_err(|_| "esta copia no es de tu cuenta o fue alterada")?;
    Ok(serde_json::from_slice(&plain)?)
}

/// Hace una copia nueva de mis datos (cifrada) y marca las de los demás como desactualizadas.
pub fn snapshot(store: &Store, me: &Identity) -> Result<u64> {
    let snap = Snapshot {
        contacts: store.contacts()?,
        devices: store.my_devices()?,
        messages: store.export_messages()?,
        ops: store.ops_after(&sync::Vector::new())?,
    };
    let path = store.replica_path(None)?;
    let tmp = path.with_extension("nueva");
    // Se cifra desde memoria: la copia nunca está sin cifrar en el disco.
    let meta = files::encrypt_bytes(&serde_json::to_vec(&snap)?, &tmp, "copia", "application/json")?;
    fs::rename(&tmp, &path)?;
    let version = store.next_hlc()?;
    store.set_my_replica(version, &wrap(me, version, &meta)?, meta.size)?;
    for h in store.holders()? {
        if h.status != "borrando" {
            store.set_holder(&h.name, h.version, "desactualizada")?;
        }
    }
    Ok(version)
}

fn connect(store: &Store, me: &Identity, h: &Holder) -> Result<Session> {
    crate::connect(store, me, h.addr.as_str(), &from_hex(&h.account)?)
}

/// Envía mi copia más reciente a quien la guarda (continúa si se cortó antes).
pub fn push(store: &Store, me: &Identity, h: &Holder) -> Result<()> {
    let (version, meta, size) = store.my_replica()?.ok_or("todavía no hay ninguna copia hecha")?;
    let mut s = connect(store, me, h)?;
    s.send(&Frame::ReplicaPut { version, meta, size })?;
    match s.recv()? {
        Some(Frame::Ack { .. }) => {}
        Some(Frame::Have { chunks, .. }) => {
            let path = store.replica_path(None)?;
            for index in chunks..files::chunk_count(size) {
                let data = B64.encode(files::read_sealed(&path, size, index)?);
                s.send(&Frame::Chunk { id: CHUNK_ID.into(), index, data })?;
            }
            s.set_timeout(Duration::from_secs(10 + size / 1_000_000))?;
            let Some(Frame::Ack { .. }) = s.recv()? else { return Err("no confirmó la copia".into()) };
        }
        Some(Frame::Bad { reason, .. }) => return Err(reason.into()),
        _ => return Err("respuesta inesperada".into()),
    }
    store.set_holder(&h.name, version, "al día")
}

/// Huella de (número al azar + archivo): demuestra tener el archivo entero sin enviarlo.
fn proof(nonce: &[u8], path: &PathBuf) -> Result<String> {
    let mut hash = Sha256::new_with_prefix(nonce);
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match f.read(&mut buf)? {
            0 => break,
            n => hash.update(&buf[..n]),
        }
    }
    Ok(to_hex(&hash.finalize()))
}

/// Comprueba que quien guarda mi copia la tiene entera y al día. Devuelve el estado.
pub fn check(store: &Store, me: &Identity, h: &Holder) -> Result<String> {
    let (version, _, _) = store.my_replica()?.ok_or("todavía no hay ninguna copia hecha")?;
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let mut s = connect(store, me, h)?;
    s.send(&Frame::ReplicaCheck { nonce: B64.encode(nonce) })?;
    s.set_timeout(Duration::from_secs(60))?;
    let status = match s.recv()? {
        Some(Frame::ReplicaProof { version: v, .. }) if v != version => "desactualizada",
        Some(Frame::ReplicaProof { proof: p, .. }) if p == proof(&nonce, &store.replica_path(None)?)? => "verificada",
        Some(Frame::ReplicaProof { .. }) => "dañada",
        Some(Frame::Bad { .. }) => "prevista", // ya no la tiene: hay que volver a enviarla
        _ => return Err("respuesta inesperada".into()),
    };
    store.set_holder(&h.name, if status == "prevista" { 0 } else { h.version }, status)?;
    Ok(status.into())
}

/// Pide que borre mi copia. Si no contesta, queda «borrando» y se reintenta más tarde.
pub fn delete(store: &Store, me: &Identity, h: &Holder) -> Result<()> {
    let result = connect(store, me, h).and_then(|mut s| {
        s.send(&Frame::ReplicaDel)?;
        match s.recv()? {
            Some(Frame::Ack { .. }) => Ok(()),
            _ => Err("no confirmó el borrado".into()),
        }
    });
    match result {
        Ok(()) => store.remove_holder(&h.name),
        Err(e) => {
            store.set_holder(&h.name, h.version, "borrando")?;
            Err(e)
        }
    }
}

/// Resumen de lo recuperado.
#[derive(Debug, PartialEq)]
pub struct Restored {
    pub contacts: usize,
    pub messages: usize,
    pub changes: usize,
}

/// Descarga mi copia de `h`, la comprueba y la incorpora. Sirve tras recuperar la cuenta en un
/// teléfono nuevo: la cuenta se demuestra en el saludo y la clave sale de la copia de recuperación.
pub fn restore(store: &Store, me: &Identity, h: &Holder) -> Result<Restored> {
    let mut s = connect(store, me, h)?;
    s.send(&Frame::ReplicaGet)?;
    let (version, meta, size) = match s.recv()? {
        Some(Frame::ReplicaPut { version, meta, size }) => (version, meta, size),
        Some(Frame::Bad { reason, .. }) => return Err(reason.into()),
        _ => return Err("respuesta inesperada".into()),
    };
    let file = unwrap(me, version, &meta)?;
    if file.size != size {
        return Err("la copia no coincide con su descripción".into());
    }
    let part = store.replica_path(None)?.with_extension("descarga");
    let _ = fs::remove_file(&part);
    for _ in 0..files::chunk_count(size) {
        let Some(Frame::Chunk { index, data, .. }) = s.recv()? else { return Err("la copia llegó incompleta".into()) };
        // Cada trozo se comprueba con la clave de la copia: uno alterado se detecta aquí.
        files::append_chunk(&part, &file, index, &B64.decode(data).map_err(|_| "trozo mal codificado")?)?;
    }
    let mut plain = Vec::new();
    let checked = files::decrypt_file(&part, &file, Some(&mut plain));
    let _ = fs::remove_file(&part);
    checked?; // huella SHA-256 comprobada
    let snap: Snapshot = serde_json::from_slice(&plain)?;
    for (name, key, addr) in &snap.contacts {
        let _ = store.add_contact(name, key, addr.as_deref()); // si ya estaba, se deja
    }
    for (name, addr) in &snap.devices {
        store.add_my_device(name, addr)?;
    }
    for m in &snap.messages {
        store.import_message(m)?;
    }
    let changes = sync::merge(store, snap.ops)?;
    // Quien me la devolvió sigue siendo un buen sitio para guardar las próximas.
    store.add_holder(&h.name, &h.account, &h.addr)?;
    store.set_holder(&h.name, version, "al día")?;
    Ok(Restored { contacts: snap.contacts.len(), messages: snap.messages.len(), changes })
}

// ---------- Lado de quien guarda copias de otros ----------

/// Copia que está llegando: (dueño, versión, datos cifrados de la copia, tamaño, recepción a medias).
pub type Incoming = (String, u64, String, u64, PathBuf);

/// `owner` quiere guardar su copia aquí.
pub fn on_put(s: &mut Session, store: &Store, me: &Identity, owner: &str, version: u64, meta: String, size: u64) -> Result<Option<Incoming>> {
    if owner != me.id() && !store.has_consent(owner)? {
        s.send(&Frame::Bad { id: CHUNK_ID.into(), reason: "no he aceptado guardar copias tuyas".into() })?;
        return Ok(None);
    }
    if size > files::MAX_SIZE {
        s.send(&Frame::Bad { id: CHUNK_ID.into(), reason: "copia demasiado grande".into() })?;
        return Ok(None);
    }
    if store.held(owner)?.is_some_and(|h| h.0 == version) {
        s.send(&Frame::Ack { id: CHUNK_ID.into() })?;
        return Ok(None);
    }
    let final_path = store.replica_path(Some(owner))?;
    let part = final_path.with_extension(format!("{version}.parcial"));
    // Una sola copia a medias por dueño: las de otras versiones se borran (auditoría, Fase 13).
    if let (Some(dir), Some(stem)) = (final_path.parent(), final_path.file_name().and_then(|n| n.to_str())) {
        for entry in fs::read_dir(dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("{stem}.")) && name.ends_with(".parcial") && entry.path() != part {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    let have = files::received_chunks(&part)?;
    s.send(&Frame::Have { id: CHUNK_ID.into(), chunks: have })?;
    let incoming = (owner.to_string(), version, meta, size, part);
    if have == files::chunk_count(size) {
        on_complete(s, store, incoming)?;
        return Ok(None);
    }
    Ok(Some(incoming))
}

/// Llega un trozo de una copia ajena. Devuelve la copia si aún faltan trozos.
pub fn on_chunk(s: &mut Session, store: &Store, incoming: Incoming, index: u64, data: &str) -> Result<Option<Incoming>> {
    let raw = B64.decode(data).map_err(|_| "trozo mal codificado")?;
    files::append_blind(&incoming.4, incoming.3, index, &raw)?;
    if index + 1 == files::chunk_count(incoming.3) {
        on_complete(s, store, incoming)?;
        return Ok(None);
    }
    Ok(Some(incoming))
}

fn on_complete(s: &mut Session, store: &Store, (owner, version, meta, size, part): Incoming) -> Result<()> {
    fs::rename(&part, store.replica_path(Some(&owner))?)?;
    store.put_held(&owner, version, &meta, size)?;
    s.send(&Frame::Ack { id: CHUNK_ID.into() })
}

pub fn on_check(s: &mut Session, store: &Store, owner: &str, nonce: &str) -> Result<()> {
    match store.held(owner)? {
        Some((version, _, _)) => {
            let nonce = B64.decode(nonce).map_err(|_| "número mal codificado")?;
            let proof = proof(&nonce, &store.replica_path(Some(owner))?)?;
            s.send(&Frame::ReplicaProof { version, proof })
        }
        None => s.send(&Frame::Bad { id: CHUNK_ID.into(), reason: "no tengo copia tuya".into() }),
    }
}

/// Devuelve la copia a su dueño (la cuenta del otro lado ya está demostrada en el saludo).
pub fn on_get(s: &mut Session, store: &Store, owner: &str) -> Result<()> {
    let Some((version, meta, size)) = store.held(owner)? else {
        return s.send(&Frame::Bad { id: CHUNK_ID.into(), reason: "no tengo ninguna copia de tu cuenta".into() });
    };
    s.send(&Frame::ReplicaPut { version, meta, size })?;
    let path = store.replica_path(Some(owner))?;
    for index in 0..files::chunk_count(size) {
        let data = B64.encode(files::read_sealed(&path, size, index)?);
        s.send(&Frame::Chunk { id: CHUNK_ID.into(), index, data })?;
    }
    Ok(())
}

pub fn on_delete(s: &mut Session, store: &Store, owner: &str) -> Result<()> {
    store.delete_held(owner)?;
    s.send(&Frame::Ack { id: CHUNK_ID.into() })
}
