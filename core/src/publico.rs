//! Colecciones públicas (pieza aparte, va junto al relé): lo único de una app que se guarda fuera
//! de los dispositivos, porque la app lo publica **a propósito** para que todos lo consulten
//! (un ranking, un catálogo, un tablón). Todo lo demás sigue privado en cada móvil.
//!
//! - Cada documento lleva su **autor**, la cuenta que lo publicó (demostrada en el saludo cifrado):
//!   nadie puede cambiar ni borrar lo de otro.
//! - Separado por app (el mismo «apellido de app» que la guía de nombres).
//! - Consultas con los mismos filtros, orden y límite que las colecciones privadas.
//! - Topes contra el abuso: tamaño por documento, documentos por cuenta y peticiones por minuto.

use crate::{
    Identity, Result, Store,
    sync::{Filter, Record},
    transport::{Frame, Session},
};
use std::time::Duration;

/// Piezas públicas que usa toda app que no elija la suya (invitación `mobilemesh:2:<clave>:<ruta>`).
// ponytail: lista fija como la de guías y guardianes; directorio que se actualice solo si crece.
// La primera es la del nubia de casa (app «Relé MobileMesh», puerto 7956).
pub const UNIVERSALES: &[&str] = &["mobilemesh:2:f342c811b44d02cc09cac35d7df54cab6c68222115ab055cc09c297bd1760860:mobilemeshjuan.duckdns.org:7956"];

/// Topes por documento, por cuenta y por consulta.
pub const MAX_CAMPOS: usize = 4096;
pub const MAX_POR_CUENTA: usize = 200;
pub const MAX_RESULTADOS: usize = 100;

/// Lo único que atiende una pieza pública.
pub fn allowed(f: &Frame) -> bool {
    matches!(f, Frame::PublicoPut { .. } | Frame::PublicoBorrar { .. } | Frame::PublicoConsulta { .. })
}

pub fn is_public(store: &Store) -> Result<bool> {
    Ok(store.setting("publico")?.as_deref() == Some("1"))
}

/// Convierte este dispositivo en pieza pública: acepta a cualquiera, pero solo para esto.
pub fn enable(store: &Store) -> Result<()> {
    store.set_setting("abierto", Some("1"))?;
    store.set_setting("publico", Some("1"))
}

/// Nombre de colección o de documento: 1–64 letras minúsculas, números, `.`, `_` o `-`.
fn nombre(n: &str) -> Option<String> {
    let n = n.trim().to_lowercase();
    ((1..=64).contains(&n.len()) && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))).then_some(n)
}

fn limite(peer: &str, que: &str, max: usize) -> bool {
    crate::access::allow(&format!("{que}:{peer}"), max, Duration::from_secs(60))
}

fn bad(s: &mut Session, reason: &str) -> Result<()> {
    s.send(&Frame::Bad { id: "publico".into(), reason: reason.into() })
}

/// Campos de un documento (objeto JSON) → registro con los valores como los guarda una app (`[v]`).
fn registro(id: &str, campos: &str) -> Option<Record> {
    let serde_json::Value::Object(m) = serde_json::from_str(campos).ok()? else { return None };
    let fields = m.into_iter().map(|(k, v)| (k, serde_json::Value::Array(vec![v]).to_string())).collect();
    Some(Record { id: id.to_string(), fields, conflicts: vec![] })
}

// ---------- Lado de la pieza ----------

pub fn on_put(s: &mut Session, store: &Store, peer: &str, app: &str, coleccion: &str, id: &str, campos: &str) -> Result<()> {
    if !is_public(store)? || !limite(peer, "publico-put", 120) {
        return bad(s, "demasiadas publicaciones; espera un minuto");
    }
    let (Ok(app), Some(col), Some(id)) = (crate::nombres::normalize_app(app), nombre(coleccion), nombre(id)) else {
        return bad(s, "colección o documento no válido");
    };
    if campos.len() > MAX_CAMPOS || registro(&id, campos).is_none() {
        return bad(s, "los campos deben ser un objeto JSON de como mucho 4 KB");
    }
    match store.public_author(&app, &col, &id)? {
        Some(autor) if autor != peer => return bad(s, "ese documento es de otra cuenta"),
        None if store.public_count(&app, &col, peer)? >= MAX_POR_CUENTA => {
            return bad(s, "has llegado al máximo de documentos públicos en esta colección");
        }
        _ => {}
    }
    store.public_put(&app, &col, &id, peer, campos)?;
    s.send(&Frame::Ack { id: "publico".into() })
}

pub fn on_delete(s: &mut Session, store: &Store, peer: &str, app: &str, coleccion: &str, id: &str) -> Result<()> {
    if !is_public(store)? || !limite(peer, "publico-put", 120) {
        return bad(s, "demasiadas peticiones; espera un minuto");
    }
    let (Ok(app), Some(col), Some(id)) = (crate::nombres::normalize_app(app), nombre(coleccion), nombre(id)) else {
        return bad(s, "colección o documento no válido");
    };
    if store.public_author(&app, &col, &id)?.is_some_and(|autor| autor != peer) {
        return bad(s, "ese documento es de otra cuenta");
    }
    store.public_delete(&app, &col, &id)?;
    s.send(&Frame::Ack { id: "publico".into() })
}

#[allow(clippy::too_many_arguments)]
pub fn on_query(
    s: &mut Session,
    store: &Store,
    peer: &str,
    app: &str,
    coleccion: &str,
    filtros: &[(String, String, String)],
    orden: Option<&str>,
    desc: bool,
    limite_: u32,
) -> Result<()> {
    if !is_public(store)? || !limite(peer, "publico-get", 300) {
        return bad(s, "demasiadas consultas; espera un minuto");
    }
    let (Ok(app), Some(col)) = (crate::nombres::normalize_app(app), nombre(coleccion)) else {
        return bad(s, "colección no válida");
    };
    let docs = store.public_docs(&app, &col)?;
    let recs: Vec<Record> = docs.iter().filter_map(|(id, _, campos)| registro(id, campos)).collect();
    // Los filtros llegan con valores JSON sueltos; se guardan como `[v]`, igual que en una app.
    let filtros: Vec<Filter> = filtros
        .iter()
        .map(|(c, op, v)| Filter { field: c.clone(), op: op.clone(), value: serde_json::from_str::<serde_json::Value>(v).map(|v| serde_json::Value::Array(vec![v]).to_string()).unwrap_or_else(|_| v.clone()) })
        .collect();
    let max = (limite_ as usize).clamp(1, MAX_RESULTADOS);
    let elegidos = crate::sync::filter_records(recs, &filtros, orden.map(|o| (o, desc)), Some(max))?;
    let lista = elegidos
        .into_iter()
        .filter_map(|r| docs.iter().find(|(id, ..)| *id == r.id).map(|(id, autor, campos)| (id.clone(), autor.clone(), campos.clone())))
        .collect();
    s.send(&Frame::PublicoLista { docs: lista })
}

// ---------- Lado de la app ----------

fn hablar(store: &Store, me: &Identity, servidor: &str, pedido: Frame) -> Result<Option<Frame>> {
    let (key, route) = crate::parse_invite(servidor)?;
    let mut s = crate::connect(store, me, &route, &crate::identity::from_hex(&key)?)?;
    s.send(&pedido)?;
    match s.recv()? {
        Some(Frame::Bad { reason, .. }) => Err(reason.into()),
        None => Err("la pieza pública no respondió".into()),
        otra => Ok(otra),
    }
}

/// Publica (o actualiza) el documento `id` de `coleccion` con `campos` (objeto JSON).
pub fn publish(store: &Store, me: &Identity, servidor: &str, coleccion: &str, id: &str, campos: &str) -> Result<()> {
    let pedido = Frame::PublicoPut { app: crate::nombres::app_de(store)?, coleccion: coleccion.into(), id: id.into(), campos: campos.into() };
    match hablar(store, me, servidor, pedido)? {
        Some(Frame::Ack { .. }) => Ok(()),
        _ => Err("respuesta inesperada de la pieza pública".into()),
    }
}

/// Borra un documento publicado por mí.
pub fn unpublish(store: &Store, me: &Identity, servidor: &str, coleccion: &str, id: &str) -> Result<()> {
    let pedido = Frame::PublicoBorrar { app: crate::nombres::app_de(store)?, coleccion: coleccion.into(), id: id.into() };
    match hablar(store, me, servidor, pedido)? {
        Some(Frame::Ack { .. }) => Ok(()),
        _ => Err("respuesta inesperada de la pieza pública".into()),
    }
}

/// Documentos públicos de `coleccion` (de todos los usuarios de la app): [(id, autor, campos JSON)].
#[allow(clippy::too_many_arguments)]
pub fn query(
    store: &Store,
    me: &Identity,
    servidor: &str,
    coleccion: &str,
    filtros: Vec<(String, String, String)>,
    orden: Option<String>,
    desc: bool,
    limite_: u32,
) -> Result<Vec<(String, String, String)>> {
    let pedido = Frame::PublicoConsulta { app: crate::nombres::app_de(store)?, coleccion: coleccion.into(), filtros, orden, desc, limite: limite_ };
    match hablar(store, me, servidor, pedido)? {
        Some(Frame::PublicoLista { docs }) => Ok(docs),
        _ => Err("respuesta inesperada de la pieza pública".into()),
    }
}
