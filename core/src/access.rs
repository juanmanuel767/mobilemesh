//! Permisos y control de acceso (Fase 10).
//!
//! - **Dispositivos retirados:** el dispositivo principal firma, con la clave de la cuenta, la lista
//!   de dispositivos que ya no pertenecen a ella (perdidos o robados), con un número de versión.
//!   Al empezar cada conexión los dos lados se mandan la lista de su cuenta: así se entera todo el
//!   que habla con cualquier dispositivo válido de esa cuenta, y nadie acepta al retirado.
//! - **Bloqueo** de contactos y **límites** de mensajes y de conexiones para frenar el spam.

use crate::{
    Identity, Result, Store,
    identity::{from_hex, from_hex_any, to_hex},
    transport::{Frame, Session},
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use crate::tiempo::Instant;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::Duration,
};

const CONTEXT: &[u8] = b"mobilemesh-retirados-v1";
const MAX_REVOKED: usize = 500;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Revocations {
    pub account: String,
    pub version: u64,
    /// Claves de dispositivo retiradas (hexadecimal).
    pub revoked: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Signed {
    list: Revocations,
    sig: String,
}

fn message(list: &Revocations) -> Result<Vec<u8>> {
    Ok([CONTEXT, &serde_json::to_vec(list)?].concat())
}

/// Firma una lista nueva (solo el principal tiene la clave de la cuenta).
pub fn sign(me: &Identity, list: &Revocations) -> Result<String> {
    let secret = me.account_secret.ok_or("solo el dispositivo principal puede retirar dispositivos")?;
    let sig = SigningKey::from_bytes(&secret).sign(&message(list)?);
    Ok(serde_json::to_string(&Signed { list: list.clone(), sig: to_hex(&sig.to_bytes()) })?)
}

/// Comprueba que la lista la firmó la cuenta `account`.
pub fn verify(data: &str, account: &str) -> Result<Revocations> {
    let s: Signed = serde_json::from_str(data)?;
    if s.list.account != account {
        return Err("la lista de retirados es de otra cuenta".into());
    }
    let key: [u8; 32] = from_hex(account)?.try_into().map_err(|_| "cuenta no válida")?;
    let sig: [u8; 64] = from_hex_any(&s.sig)?.try_into().map_err(|_| "firma no válida")?;
    VerifyingKey::from_bytes(&key)
        .and_then(|k| k.verify_strict(&message(&s.list)?, &Signature::from_bytes(&sig)))
        .map_err(|_| "la lista de retirados no está firmada por su cuenta")?;
    Ok(s.list)
}

/// Retira un dispositivo de mi cuenta: nueva versión de la lista, firmada y guardada.
pub fn revoke(store: &Store, me: &Identity, device: &str) -> Result<Revocations> {
    from_hex(device)?;
    if device == to_hex(&me.public) {
        return Err("no puedes retirar el dispositivo que estás usando".into());
    }
    let mut list = match store.revocations(&me.id())? {
        Some((_, data)) => verify(&data, &me.id())?,
        None => Revocations { account: me.id(), version: 0, revoked: vec![] },
    };
    if !list.revoked.iter().any(|d| d == device) {
        // La lista viaja en un solo paquete: tiene que caber.
        if list.revoked.len() >= MAX_REVOKED {
            return Err("ya hay demasiados dispositivos retirados en esta cuenta".into());
        }
        list.revoked.push(device.into());
    }
    list.version += 1;
    store.set_revocations(&me.id(), list.version, &sign(me, &list)?)?;
    Ok(list)
}

/// Primer paso de toda conexión ya autenticada: cada lado manda la lista de su cuenta y recibe
/// la del otro. Después se rechaza si el otro está bloqueado o es un dispositivo retirado.
pub fn exchange(s: &mut Session, store: &Store, me: &Identity) -> Result<()> {
    let mine = store.revocations(&me.id())?.map(|(_, d)| d);
    s.send(&Frame::Rev { list: mine })?;
    let peer = to_hex(&s.peer);
    match s.recv()? {
        Some(Frame::Rev { list: Some(data) }) => {
            let list = verify(&data, &peer)?;
            if store.revocations(&peer)?.is_none_or(|(v, _)| list.version > v) {
                store.set_revocations(&peer, list.version, &data)?;
            }
        }
        Some(Frame::Rev { list: None }) => {}
        _ => return Err("el otro lado no siguió el protocolo".into()),
    }
    let device = to_hex(&s.device);
    if store.is_revoked(&peer, &device)? {
        return Err("ese dispositivo fue retirado por el dueño de la cuenta".into());
    }
    if peer == me.id() {
        store.seen_device(&device)?;
    }
    // Para poder leer después cartas de este dispositivo (buzón en el relé).
    store.set_device_account(&device, &peer)?;
    if store.is_blocked(&peer)? {
        return Err("contacto bloqueado".into());
    }
    Ok(())
}

// ---------- Límites contra el spam ----------

static WINDOWS: Mutex<Option<HashMap<String, VecDeque<Instant>>>> = Mutex::new(None);

/// ¿Se permite otra acción de `key` (como mucho `max` por `window`)? Cuenta la acción si se permite.
pub fn allow(key: &str, max: usize, window: Duration) -> bool {
    let mut guard = WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let now = Instant::now();
    // Que la tabla no crezca sin fin con direcciones distintas (auditoría, Fase 13).
    if map.len() > 10_000 {
        map.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) <= window));
    }
    let q = map.entry(key.into()).or_default();
    while q.front().is_some_and(|t| now.duration_since(*t) > window) {
        q.pop_front();
    }
    if q.len() >= max {
        return false;
    }
    q.push_back(now);
    true
}

/// Mensajes por minuto que se aceptan de un mismo contacto.
pub const MESSAGES_PER_MINUTE: usize = 60;
/// Conexiones por minuto que se aceptan desde una misma dirección IP (antes del saludo).
pub const CONNECTIONS_PER_MINUTE: usize = 30;
/// Prefijo de los rechazos por límite: el remitente no lo da por error, lo deja pendiente.
pub const LIMIT: &str = "LÍMITE";
