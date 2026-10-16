//! Guardián (pieza aparte, va junto al relé): un MobileMesh sin pantalla que guarda, para
//! cualquiera, lo necesario para recuperarse tras perder el teléfono:
//!
//! - el **código de recuperación** de la cuenta (`MM3-`), que ya va cifrado con la contraseña del
//!   usuario. Se encuentra con usuario + contraseña: el guardián solo ve una huella (Argon2id) de
//!   los dos, nunca el usuario ni la contraseña;
//! - la **copia cifrada de los datos** (la de la Fase 8, hasta `MAX_COPIA` por cuenta), cifrada con
//!   la clave de la cuenta.
//!
//! No puede leer nada de lo que guarda. Si se rompe, se cambia por otro sin tocar el relé.

use crate::{
    Identity, Result, Store,
    identity::{Plain, to_hex},
    recovery,
    transport::{Frame, Session},
};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

/// Guardianes que usa toda app que no elija los suyos (invitación `mobilemesh:2:<clave>:<ruta>`).
// ponytail: lista fija como la de relés; directorio que se actualice solo si crece.
// El primero es el del nubia de casa (app «Relé MobileMesh», puerto 7954).
pub const UNIVERSALES: &[&str] = &["mobilemesh:2:5dc0a5fc029b44b53ca14ec0fe4236991cde1617bfc220159bdd373f5af99787:mobilemeshjuan.duckdns.org:7954"];

/// Espacio por cuenta para la copia de datos.
pub const MAX_COPIA: u64 = 50 * 1024 * 1024;

/// Días sin señales del dueño tras los que el guardián borra su copia de datos. Los códigos de
/// cuenta no caducan: ocupan muy poco y hacen falta justo cuando el teléfono se perdió.
pub const CADUCA_DIAS: i64 = 90;

/// Contraseña mínima para guardar el código en un guardián: cualquiera puede probar contraseñas
/// contra él, así que se pide más que para un código guardado en papel.
pub const MIN_PASS: usize = 12;

/// ¿Este dispositivo es un guardián público?
pub fn is_guardian(store: &Store) -> Result<bool> {
    Ok(store.setting("guardian")?.as_deref() == Some("1"))
}

/// Convierte este dispositivo en guardián: acepta a cualquiera (como un ayudante abierto).
pub fn enable(store: &Store) -> Result<()> {
    store.set_setting("abierto", Some("1"))?;
    store.set_setting("guardian", Some("1"))
}

/// Dónde se guarda el código: huella de usuario + contraseña con Argon2id (cada intento de
/// adivinarla cuesta 64 MiB y tiempo), así el guardián no aprende ninguno de los dos.
pub fn locator(user: &str, pass: &str) -> Result<String> {
    let user = user.trim().to_lowercase();
    if user.is_empty() {
        return Err("falta el nombre de usuario".into());
    }
    if pass.chars().count() < MIN_PASS {
        return Err(format!("la contraseña debe tener al menos {MIN_PASS} caracteres").into());
    }
    let salt = Sha256::new_with_prefix(b"mobilemesh-guardian-v1").chain_update(user.as_bytes()).finalize();
    let k = recovery::key(pass, &salt[..16], true)?;
    Ok(to_hex(&Sha256::new_with_prefix(b"localizador").chain_update(k).finalize()))
}

fn valid_locator(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

// ---------- Lado del guardián ----------

/// Guarda el código de recuperación de `peer`. Solo su dueño puede cambiarlo después.
pub fn on_put(s: &mut Session, store: &Store, peer: &str, id: &str, code: &str) -> Result<()> {
    let ok = is_guardian(store)?
        && valid_locator(id)
        && code.len() <= 200
        && code.to_lowercase().starts_with("mm3")
        && crate::access::allow(&format!("cuenta-put:{peer}"), 5, Duration::from_secs(60));
    let key = format!("cuenta:{id}");
    let owner_ok = match store.setting(&key)? {
        Some(v) => v.split_once('\n').is_some_and(|(owner, _)| owner == peer),
        None => true,
    };
    if !(ok && owner_ok) {
        s.send(&Frame::Bad { id: "cuenta".into(), reason: "el guardián no aceptó el código".into() })?;
        return Ok(());
    }
    store.set_setting(&key, Some(&format!("{peer}\n{code}")))?;
    s.send(&Frame::Ack { id: "cuenta".into() })?;
    Ok(())
}

/// Devuelve el código guardado en `id` (o ninguno). Con límites para frenar a quien pruebe
/// contraseñas a lo loco.
pub fn on_get(s: &mut Session, store: &Store, peer: &str, id: &str) -> Result<()> {
    if !is_guardian(store)?
        || !valid_locator(id)
        || !crate::access::allow(&format!("cuenta-get:{peer}"), 5, Duration::from_secs(60))
        || !crate::access::allow("cuenta-get", 60, Duration::from_secs(60))
    {
        s.send(&Frame::Bad { id: "cuenta".into(), reason: "demasiados intentos; espera un minuto".into() })?;
        return Ok(());
    }
    let code = store.setting(&format!("cuenta:{id}"))?.and_then(|v| v.split_once('\n').map(|(_, c)| c.to_string()));
    s.send(&Frame::Cuenta { code })?;
    Ok(())
}

/// Borra las copias de datos abandonadas (sin señales desde `before`, segundos Unix). Devuelve
/// cuántas borró.
pub fn purge_before(store: &Store, before: i64) -> Result<usize> {
    let viejas = store.stale_held(before)?;
    for owner in &viejas {
        store.delete_held(owner)?;
    }
    Ok(viejas.len())
}

/// Borra las copias sin señales en `CADUCA_DIAS`.
pub fn purge(store: &Store) -> Result<usize> {
    let ahora = crate::tiempo::SystemTime::now().duration_since(crate::tiempo::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    purge_before(store, ahora - CADUCA_DIAS * 86_400)
}

// ---------- Lado del usuario ----------

/// Guarda en el guardián (`route`, cuenta `key`) mi código de recuperación cifrado con `pass`.
pub fn save_account(store: &Store, me: &Identity, route: &str, key: &str, user: &str, pass: &str) -> Result<()> {
    let id = locator(user, pass)?;
    let code = recovery::export(me, pass)?;
    let mut s = crate::connect(store, me, route, &crate::identity::from_hex(key)?)?;
    s.send(&Frame::CuentaPut { id, code })?;
    match s.recv()? {
        Some(Frame::Ack { .. }) => Ok(()),
        Some(Frame::Bad { reason, .. }) => Err(reason.into()),
        _ => Err("el guardián no respondió".into()),
    }
}

/// En un teléfono nuevo: busca en el guardián el código con usuario + contraseña y recupera la
/// cuenta. `tmp` es una carpeta vacía para la identidad de paso con la que se habla con él.
pub fn find_account(tmp: &Path, route: &str, key: &str, user: &str, pass: &str) -> Result<Identity> {
    let id = locator(user, pass)?;
    let _ = std::fs::remove_dir_all(tmp);
    let paso = Identity::create(tmp, &Plain)?;
    let code = {
        let store = Store::open(tmp, paso.db_key())?;
        let mut s = crate::connect(&store, &paso, route, &crate::identity::from_hex(key)?)?;
        s.send(&Frame::CuentaGet { id })?;
        match s.recv()? {
            Some(Frame::Cuenta { code: Some(c) }) => c,
            Some(Frame::Cuenta { code: None }) => return Err("usuario o contraseña incorrectos".into()),
            Some(Frame::Bad { reason, .. }) => return Err(reason.into()),
            _ => return Err("el guardián no respondió".into()),
        }
    };
    let _ = std::fs::remove_dir_all(tmp);
    recovery::import(&code, pass)
}
