//! Cifrado de extremo a extremo por mensaje: Double Ratchet (Olm de vodozemac, biblioteca auditada).
//! Cada mensaje usa una clave distinta que va cambiando. Va dentro del canal Noise, pero no depende
//! de él: es lo que permitirá que un relé u otro teléfono guarde mensajes sin poder leerlos.

use crate::{Result, Store, files::FileMeta};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard};
use vodozemac::{
    Curve25519PublicKey,
    olm::{Account, OlmMessage, Session, SessionConfig},
};

pub const KEY_CHANGED: &str = "la clave de cifrado de ese dispositivo cambió; no se acepta hasta comprobarlo con su dueño";

/// El estado del ratchet se lee, cambia y guarda bajo este candado: si enviar y recibir lo tocaran
/// a la vez, uno pisaría al otro y la sesión quedaría desincronizada.
// ponytail: candado del proceso (basta con una app); con varios procesos harían falta transacciones.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Lo que va cifrado: el identificador viaja dentro para que nadie pueda cambiarlo por fuera.
#[derive(Serialize, Deserialize)]
struct Inner {
    id: String,
    body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<FileMeta>,
}

/// Claves que publica el receptor para que le abran una sesión: (identidad Olm, clave de un solo uso).
pub fn publish_keys(store: &Store) -> Result<(String, String)> {
    let _g = lock();
    let mut account = store.olm_account()?;
    let otk = account.generate_one_time_keys(1).created.pop().ok_or("no se pudo generar la clave de un solo uso")?;
    account.mark_keys_as_published();
    store.save_account(&account)?;
    Ok((account.curve25519_key().to_base64(), otk.to_base64()))
}

pub fn has_session(store: &Store, device: &str) -> Result<bool> {
    Ok(!store.sessions(device)?.is_empty())
}

/// Abre una sesión con el dispositivo `device` usando las claves que nos envió.
pub fn start_session(store: &Store, device: &str, identity: &str, one_time: &str) -> Result<()> {
    let _g = lock();
    store.pin(device, identity)?;
    let session = store.olm_account()?.create_outbound_session(
        SessionConfig::version_1(),
        Curve25519PublicKey::from_base64(identity)?,
        Curve25519PublicKey::from_base64(one_time)?,
    )?;
    store.save_session(device, &session)
}

/// Olvida las sesiones con un dispositivo (cuando él dice haberlas perdido); se rehacen al reintentar.
pub fn forget(store: &Store, device: &str) -> Result<()> {
    let _g = lock();
    store.drop_sessions(device)
}

/// Cifra un mensaje para `device`. Devuelve (tipo Olm, texto cifrado en base64).
pub fn encrypt(store: &Store, device: &str, id: &str, body: &str, file: Option<&FileMeta>) -> Result<(usize, String)> {
    let _g = lock();
    let mut session = store.sessions(device)?.into_iter().next().ok_or("no hay sesión cifrada con ese dispositivo")?;
    let plain = serde_json::to_vec(&Inner { id: id.into(), body: body.into(), file: file.cloned() })?;
    let (kind, ct) = session.encrypt(plain)?.to_parts();
    store.save_session(device, &session)?;
    Ok((kind, B64.encode(ct)))
}

/// Mensaje descifrado: texto y, si lo hay, archivo adjunto.
pub type Plain = (String, Option<FileMeta>);

/// Descifra un mensaje de `device`. `None` = no hay sesión con la que leerlo (el otro debe abrir una).
/// Nada se guarda hasta comprobar que el mensaje entero es válido: un mensaje falso no gasta claves.
pub fn decrypt(store: &Store, device: &str, id: &str, kind: usize, body: &str) -> Result<Option<Plain>> {
    let _g = lock();
    let msg = OlmMessage::from_parts(kind, &B64.decode(body).map_err(|_| "mensaje mal codificado")?)?;
    let sessions = store.sessions(device)?;
    let mut found: Option<(Vec<u8>, Session, Option<Account>)> = None;
    for mut s in sessions {
        if let Ok(p) = s.decrypt(&msg) {
            found = Some((p, s, None));
            break;
        }
    }
    if found.is_none()
        && let OlmMessage::PreKey(pre) = &msg
    {
        let identity = pre.identity_key().to_base64();
        if store.pinned(device)?.is_some_and(|p| p != identity) {
            return Err(KEY_CHANGED.into());
        }
        let mut account = store.olm_account()?;
        if let Ok(created) = account.create_inbound_session(SessionConfig::version_1(), pre.identity_key(), pre) {
            found = Some((created.plaintext, created.session, Some(account)));
        }
    }
    // Ninguna sesión lo abre: se pide al otro que abra una nueva. Así el canal nunca queda roto
    // para siempre (auditoría, Fase 13). Solo puede pedirlo quien ya pasó el saludo autenticado.
    let Some((plain, session, account)) = found else {
        return Ok(None);
    };
    let inner: Inner = serde_json::from_slice(&plain)?;
    if inner.id != id {
        return Err("el identificador del mensaje no coincide con su contenido cifrado".into());
    }
    if let Some(account) = account {
        store.pin(device, &session.session_keys().identity_key.to_base64())?;
        store.save_account(&account)?; // la clave de un solo uso queda gastada
    }
    store.save_session(device, &session)?;
    Ok(Some((inner.body, inner.file)))
}
