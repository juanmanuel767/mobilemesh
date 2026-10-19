//! Guía de nombres (pieza aparte, va junto al relé): un MobileMesh sin pantalla que hace de
//! «agenda pública», para que las apps funcionen como cualquier base de datos con usuarios:
//!
//! - **registrar**: «soy `juan`» → guarda `juan → mi invitación`. La cuenta se demuestra en el
//!   saludo cifrado y la invitación debe ser de esa misma cuenta: nadie puede registrar a otro ni
//!   quitarle el nombre a su dueño (el primero que lo registra se lo queda);
//! - **buscar**: «¿quién es `ana`?» → su invitación (datos públicos);
//! - **solicitudes**: «quiero hablar con `ana`» → la guía se lo apunta a ana, y la app de ana, al
//!   pedir sus solicitudes, me añade sola como contacto. Así ana recibe mis mensajes sin que nadie
//!   tenga que pasar códigos.
//!
//! Solo ve nombres e invitaciones (datos públicos). Mensajes y datos nunca pasan por aquí.

use crate::{
    Identity, Result, Store,
    transport::{Frame, Session},
};
use std::time::Duration;

/// Guías que usa toda app que no elija la suya (invitación `mobilemesh:2:<clave>:<ruta>`).
// ponytail: lista fija como la de relés y guardianes; directorio que se actualice solo si crece.
// La primera es la del nubia de casa (app «Relé MobileMesh», puerto 7955).
pub const UNIVERSALES: &[&str] = &["mobilemesh:2:1ae27549992df248c8856917f94a95cbda45219011716cebc68696600bf53369:mobilemeshjuan.duckdns.org:7955"];

/// Solicitudes guardadas como máximo por destinatario (las más viejas se descartan).
pub const MAX_SOLICITUDES: usize = 50;

/// Lo único que una guía atiende.
pub fn allowed(f: &Frame) -> bool {
    matches!(f, Frame::NombrePut { .. } | Frame::NombreGet { .. } | Frame::NombreSolicitar { .. } | Frame::NombreSolicitudes)
}

pub fn is_guide(store: &Store) -> Result<bool> {
    Ok(store.setting("guia_nombres")?.as_deref() == Some("1"))
}

/// Convierte este dispositivo en guía: acepta a cualquiera, pero solo para nombres.
pub fn enable(store: &Store) -> Result<()> {
    store.set_setting("abierto", Some("1"))?;
    store.set_setting("guia_nombres", Some("1"))
}

/// Nombre normalizado: 3–30 letras minúsculas, números, `.`, `_` o `-`.
pub fn normalize(nombre: &str) -> Result<String> {
    let n = nombre.trim().to_lowercase();
    let ok = (3..=30).contains(&n.chars().count()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
    if ok { Ok(n) } else { Err("el nombre debe tener de 3 a 30 letras sin tildes, números, «.», «_» o «-»".into()) }
}

fn bad(s: &mut Session, reason: &str) -> Result<()> {
    s.send(&Frame::Bad { id: "nombre".into(), reason: reason.into() })
}

/// Por cuenta, nunca un tope común: uno compartido lo agota un solo abusón y bloquea a todos.
fn limite(peer: &str, que: &str) -> bool {
    crate::access::allow(&format!("{que}:{peer}"), 60, Duration::from_secs(60))
}

// ---------- Lado de la guía ----------

/// Registra (o actualiza la invitación de) `nombre` para `peer`.
pub fn on_put(s: &mut Session, store: &Store, peer: &str, nombre: &str, invitacion: &str) -> Result<()> {
    if !is_guide(store)? || !limite(peer, "nombre-put") {
        return bad(s, "demasiados intentos; espera un minuto");
    }
    let Ok(nombre) = normalize(nombre) else { return bad(s, "nombre no válido") };
    match crate::parse_invite(invitacion) {
        Ok((key, _)) if key == peer && invitacion.len() <= 600 => {}
        _ => return bad(s, "la invitación no es de tu cuenta"),
    }
    let clave = format!("nombre:{nombre}");
    if store.setting(&clave)?.is_some_and(|v| v.split_once('\n').is_none_or(|(dueno, _)| dueno != peer)) {
        return bad(s, "ese nombre ya lo usa otra persona");
    }
    // Una cuenta, un nombre: si tenía otro, se libera.
    if let Some(viejo) = store.setting(&format!("cuenta-nombre:{peer}"))?.filter(|v| *v != nombre) {
        store.set_setting(&format!("nombre:{viejo}"), None)?;
    }
    store.set_setting(&clave, Some(&format!("{peer}\n{}", invitacion.trim())))?;
    store.set_setting(&format!("cuenta-nombre:{peer}"), Some(&nombre))?;
    s.send(&Frame::Ack { id: "nombre".into() })
}

fn invitacion_de(store: &Store, nombre: &str) -> Result<Option<String>> {
    Ok(store.setting(&format!("nombre:{nombre}"))?.and_then(|v| v.split_once('\n').map(|(_, i)| i.to_string())))
}

pub fn on_get(s: &mut Session, store: &Store, peer: &str, nombre: &str) -> Result<()> {
    if !is_guide(store)? || !limite(peer, "nombre-get") {
        return bad(s, "demasiadas búsquedas; espera un minuto");
    }
    let invitacion = match normalize(nombre) {
        Ok(n) => invitacion_de(store, &n)?,
        Err(_) => None,
    };
    s.send(&Frame::Nombre { invitacion })
}

/// Apunta a `nombre` que `peer` (con su nombre registrado) quiere hablar con él.
pub fn on_request(s: &mut Session, store: &Store, peer: &str, nombre: &str) -> Result<()> {
    if !is_guide(store)? || !limite(peer, "nombre-sol") {
        return bad(s, "demasiadas solicitudes; espera un minuto");
    }
    let Some(yo) = store.setting(&format!("cuenta-nombre:{peer}"))? else { return bad(s, "primero registra tu nombre") };
    let Some(destino) = normalize(nombre).ok().and_then(|n| store.setting(&format!("nombre:{n}")).ok().flatten()) else {
        return bad(s, "ese nombre no existe");
    };
    let (cuenta_destino, _) = destino.split_once('\n').ok_or("registro dañado")?;
    let clave = format!("solicitudes:{cuenta_destino}");
    let mut lista: Vec<String> = store.setting(&clave)?.map(|v| serde_json::from_str(&v)).transpose()?.unwrap_or_default();
    lista.retain(|n| *n != yo);
    lista.push(yo);
    let sobran = lista.len().saturating_sub(MAX_SOLICITUDES);
    lista.drain(..sobran);
    store.set_setting(&clave, Some(&serde_json::to_string(&lista)?))?;
    s.send(&Frame::Ack { id: "nombre".into() })
}

/// Entrega (y borra) las solicitudes de `peer`: [(nombre, invitación actual)].
pub fn on_requests(s: &mut Session, store: &Store, peer: &str) -> Result<()> {
    if !is_guide(store)? || !limite(peer, "nombre-sols") {
        return bad(s, "demasiadas consultas; espera un minuto");
    }
    let clave = format!("solicitudes:{peer}");
    let nombres: Vec<String> = store.setting(&clave)?.map(|v| serde_json::from_str(&v)).transpose()?.unwrap_or_default();
    let mut lista = vec![];
    for n in nombres {
        if let Some(i) = invitacion_de(store, &n)? {
            lista.push((n, i));
        }
    }
    store.set_setting(&clave, None)?;
    s.send(&Frame::Solicitudes { lista })
}

// ---------- Lado del usuario ----------

fn hablar(store: &Store, me: &Identity, guia: &str, pedido: Frame) -> Result<Option<Frame>> {
    let (key, route) = crate::parse_invite(guia)?;
    let mut s = crate::connect(store, me, &route, &crate::identity::from_hex(&key)?)?;
    s.send(&pedido)?;
    match s.recv()? {
        Some(Frame::Bad { reason, .. }) => Err(reason.into()),
        None => Err("la guía de nombres no respondió".into()),
        otra => Ok(otra),
    }
}

/// Registra `nombre` con mi invitación en la guía `guia`.
pub fn register(store: &Store, me: &Identity, guia: &str, nombre: &str, invitacion: &str) -> Result<()> {
    let nombre = normalize(nombre)?;
    match hablar(store, me, guia, Frame::NombrePut { nombre, invitacion: invitacion.into() })? {
        Some(Frame::Ack { .. }) => Ok(()),
        _ => Err("respuesta inesperada de la guía".into()),
    }
}

/// Invitación de `nombre`, si existe.
pub fn find(store: &Store, me: &Identity, guia: &str, nombre: &str) -> Result<Option<String>> {
    match hablar(store, me, guia, Frame::NombreGet { nombre: normalize(nombre)? })? {
        Some(Frame::Nombre { invitacion }) => Ok(invitacion),
        _ => Err("respuesta inesperada de la guía".into()),
    }
}

/// Pide a la guía que avise a `nombre` de que quiero hablar con él.
pub fn request(store: &Store, me: &Identity, guia: &str, nombre: &str) -> Result<()> {
    match hablar(store, me, guia, Frame::NombreSolicitar { nombre: normalize(nombre)? })? {
        Some(Frame::Ack { .. }) => Ok(()),
        _ => Err("respuesta inesperada de la guía".into()),
    }
}

/// Mis solicitudes pendientes: [(nombre, invitación)]. La guía las borra al entregarlas.
pub fn requests(store: &Store, me: &Identity, guia: &str) -> Result<Vec<(String, String)>> {
    match hablar(store, me, guia, Frame::NombreSolicitudes)? {
        Some(Frame::Solicitudes { lista }) => Ok(lista),
        _ => Err("respuesta inesperada de la guía".into()),
    }
}
