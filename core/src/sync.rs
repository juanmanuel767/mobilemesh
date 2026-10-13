//! Datos sincronizados entre los dispositivos de una cuenta (Fase 7).
//!
//! Cada cambio es una **operación** numerada por el dispositivo que la hizo (dispositivo, nº) y se
//! guarda primero en local. Para sincronizar, cada lado dice hasta qué número tiene de cada
//! dispositivo (vector de versiones) y recibe solo lo que le falta. Las operaciones se pueden
//! recibir varias veces o en otro orden: el resultado es el mismo.
//!
//! Reglas según el tipo de dato:
//! - [`Policy::Lww`] (perfiles, ajustes): cada campo se queda con el cambio más reciente según un
//!   reloj híbrido (no depende de que los relojes de los teléfonos estén en hora).
//! - [`Policy::Strict`] (saldos, inventario, dinero): si dos dispositivos cambian el mismo campo sin
//!   haber visto el cambio del otro, **no se mezcla**: queda en conflicto hasta que alguien elija.

use crate::{Result, Store};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Policy {
    Lww,
    Strict,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Op {
    pub device: String,
    pub seq: u64,
    pub collection: String,
    pub record: String,
    pub field: String,
    /// `None` = borrar el campo.
    pub value: Option<String>,
    /// Reloj híbrido: milisegundos << 16 | contador. Ordena los cambios aunque los relojes difieran.
    pub hlc: u64,
    /// Versión (hlc) del campo que veía quien hizo el cambio. Sirve para detectar cambios a la vez.
    pub base: u64,
    pub policy: Policy,
    /// Conflictos que quien escribió tenía delante y resuelve con este cambio (dispositivo, nº).
    /// Un cambio encadenado de alguien que no vio el conflicto no lo cierra.
    #[serde(default)]
    pub resolves: Vec<(String, u64)>,
    /// Cuenta que hizo el cambio. Un contacto solo puede mandar cambios suyos (Fase 10).
    #[serde(default)]
    pub author: String,
}

/// Estado de un campo: valor, versión y quién la escribió.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldState {
    pub value: Option<String>,
    pub hlc: u64,
    pub device: String,
    pub seq: u64,
}

/// Vector de versiones: hasta qué número de operación se tiene de cada dispositivo.
pub type Vector = HashMap<String, u64>;

// ponytail: tamaño máximo de un valor; los archivos van por la Fase 6, no por aquí.
pub const MAX_VALUE: usize = 8 * 1024;

pub fn now_hlc() -> u64 {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    ms << 16
}

/// Cambio hecho en este dispositivo: se guarda y se aplica en local al momento.
pub fn put(store: &Store, me: &str, policy: Policy, collection: &str, record: &str, field: &str, value: Option<&str>) -> Result<()> {
    put_as(store, me, "", policy, collection, record, field, value)
}

/// Como `put`, apuntando la cuenta autora (necesario en colecciones compartidas con contactos).
#[allow(clippy::too_many_arguments)]
pub fn put_as(store: &Store, me: &str, author: &str, policy: Policy, collection: &str, record: &str, field: &str, value: Option<&str>) -> Result<()> {
    if value.is_some_and(|v| v.len() > MAX_VALUE) {
        return Err(format!("el valor pasa del límite de {MAX_VALUE} bytes").into());
    }
    if collection.is_empty() || record.is_empty() || field.is_empty() {
        return Err("colección, registro y campo no pueden estar vacíos".into());
    }
    let base = store.field(collection, record, field)?.map_or(0, |f| f.hlc);
    // En modo estricto, escribir sobre un campo en conflicto es elegir: resuelve lo que se ve ahora.
    let resolves = match policy {
        Policy::Strict => store.conflict_keys(collection, record, field)?,
        Policy::Lww => vec![],
    };
    let op = Op {
        author: author.into(),
        resolves,
        device: me.into(),
        seq: store.next_seq(me)?,
        collection: collection.into(),
        record: record.into(),
        field: field.into(),
        value: value.map(Into::into),
        hlc: store.next_hlc()?,
        base,
        policy,
    };
    store.atomic(|| {
        if !store.insert_op(&op)? {
            return Err("número de operación repetido; inténtalo otra vez".into());
        }
        apply(store, &op)
    })
}

/// Números de operación por encima de esto no se aceptan (caben exactos en cualquier plataforma).
const MAX_SEQ: u64 = 1 << 53;
/// Salto máximo respecto a lo último visto de un dispositivo.
const MAX_GAP: u64 = 1_000_000;
/// Cuánto puede ir adelantado el reloj de un cambio recibido.
const MAX_SKEW_MS: u64 = 24 * 3600 * 1000;

/// Comprobaciones de cualquier cambio recibido, venga de quien venga (auditoría, Fase 13):
/// números y reloj razonables y valores de tamaño permitido.
pub fn sane(store: &Store, op: &Op) -> Result<bool> {
    Ok(op.seq >= 1
        && op.seq < MAX_SEQ
        && op.seq <= store.max_seq(&op.device)?.saturating_add(MAX_GAP)
        && op.hlc <= now_hlc().saturating_add(MAX_SKEW_MS << 16)
        && op.value.as_ref().is_none_or(|v| v.len() <= MAX_VALUE)
        && !op.collection.is_empty()
        && !op.record.is_empty()
        && !op.field.is_empty())
}

/// Incorpora operaciones recibidas de otro dispositivo. Devuelve cuántas eran nuevas.
pub fn merge(store: &Store, mut ops: Vec<Op>) -> Result<usize> {
    // Por reloj: lo que alguien hizo después de ver otro cambio llega detrás de ese cambio.
    ops.sort_by(|a, b| (a.hlc, &a.device).cmp(&(b.hlc, &b.device)));
    let ops: Vec<Op> = ops.into_iter().filter(|op| sane(store, op).unwrap_or(false)).collect();
    // Toda la tanda de una vez: mucho más rápido que operación a operación.
    store.atomic(|| {
        let mut new = 0;
        for op in &ops {
            if store.insert_op(op)? {
                apply(store, op)?;
                new += 1;
            }
        }
        Ok(new)
    })
}

fn apply(store: &Store, op: &Op) -> Result<()> {
    store.observe_hlc(op.hlc)?;
    let state = FieldState { value: op.value.clone(), hlc: op.hlc, device: op.device.clone(), seq: op.seq };
    let Some(cur) = store.field(&op.collection, &op.record, &op.field)? else {
        return store.set_field(&op.collection, &op.record, &op.field, &state);
    };
    let newer = (op.hlc, &op.device) > (cur.hlc, &cur.device);
    match op.policy {
        Policy::Lww => {
            if newer {
                store.set_field(&op.collection, &op.record, &op.field, &state)?;
            }
        }
        // Quien escribió veía la versión actual: es un cambio encadenado. Solo cierra los conflictos
        // que tenía delante; si había otros que no vio, siguen abiertos.
        Policy::Strict if op.base == cur.hlc => {
            store.set_field(&op.collection, &op.record, &op.field, &state)?;
            for (device, seq) in &op.resolves {
                store.clear_conflict(&op.collection, &op.record, &op.field, device, *seq)?;
            }
        }
        // Dos cambios hechos a la vez sobre lo mismo: se guardan los dos y se avisa; nadie elige solo.
        Policy::Strict => {
            let (winner, loser) = if newer { (state, (cur.device.clone(), cur.seq)) } else { (cur, (op.device.clone(), op.seq)) };
            store.set_field(&op.collection, &op.record, &op.field, &winner)?;
            store.add_conflict(&op.collection, &op.record, &op.field, &loser.0, loser.1)?;
        }
    }
    Ok(())
}

/// Un registro con sus campos y, si los hay, los valores en conflicto de cada campo.
#[derive(Debug, PartialEq)]
pub struct Record {
    pub id: String,
    pub fields: Vec<(String, String)>,
    /// (campo, valores alternativos) que esperan que alguien elija.
    pub conflicts: Vec<(String, Vec<String>)>,
}

pub fn records(store: &Store, collection: &str) -> Result<Vec<Record>> {
    let mut out: Vec<Record> = Vec::new();
    for (record, field, state) in store.fields(collection)? {
        if out.last().is_none_or(|r| r.id != record) {
            out.push(Record { id: record.clone(), fields: vec![], conflicts: vec![] });
        }
        let r = out.last_mut().expect("recién añadido");
        let alternatives = store.conflict_values(collection, &record, &field)?;
        if let Some(v) = state.value {
            r.fields.push((field.clone(), v));
        }
        if !alternatives.is_empty() {
            r.conflicts.push((field, alternatives));
        }
    }
    out.retain(|r| !r.fields.is_empty() || !r.conflicts.is_empty());
    Ok(out)
}

/// Operaciones propias que algún dispositivo propio conocido aún no ha confirmado.
pub fn unsynced(store: &Store, me: &str) -> Result<u64> {
    let mine = store.vector()?.get(me).copied().unwrap_or(0);
    let peers = store.peer_vectors()?;
    let confirmed = peers.values().map(|v| v.get(me).copied().unwrap_or(0)).min().unwrap_or(0);
    Ok(mine.saturating_sub(confirmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(tag: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("mm-sync-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Store::open(&dir, [3u8; 32]).unwrap()
    }

    /// Sincroniza dos dispositivos en las dos direcciones, como haría la red.
    fn exchange(a: &Store, b: &Store) {
        let (va, vb) = (a.vector().unwrap(), b.vector().unwrap());
        let (to_b, to_a) = (a.ops_after(&vb).unwrap(), b.ops_after(&va).unwrap());
        merge(b, to_b).unwrap();
        merge(a, to_a).unwrap();
    }

    fn get(s: &Store, c: &str, r: &str, f: &str) -> Option<String> {
        s.field(c, r, f).unwrap().and_then(|x| x.value)
    }

    #[test]
    fn perfil_cada_campo_con_el_cambio_mas_reciente() {
        let (a, b) = (store("lww-a"), store("lww-b"));
        // Sin conexión: cada uno cambia cosas distintas y los dos cambian la ciudad.
        put(&a, "A", Policy::Lww, "perfil", "yo", "nombre", Some("Juan")).unwrap();
        put(&b, "B", Policy::Lww, "perfil", "yo", "estado", Some("de viaje")).unwrap();
        put(&a, "A", Policy::Lww, "perfil", "yo", "ciudad", Some("Bogotá")).unwrap();
        put(&b, "B", Policy::Lww, "perfil", "yo", "ciudad", Some("Medellín")).unwrap(); // después
        exchange(&a, &b);
        for s in [&a, &b] {
            assert_eq!(get(s, "perfil", "yo", "nombre").as_deref(), Some("Juan"));
            assert_eq!(get(s, "perfil", "yo", "estado").as_deref(), Some("de viaje"));
            assert_eq!(get(s, "perfil", "yo", "ciudad").as_deref(), Some("Medellín"));
        }
        assert_eq!(records(&a, "perfil").unwrap(), records(&b, "perfil").unwrap());
    }

    #[test]
    fn saldo_cambiado_a_la_vez_queda_en_conflicto_hasta_elegir() {
        let (a, b) = (store("st-a"), store("st-b"));
        put(&a, "A", Policy::Strict, "caja", "saldo", "valor", Some("100")).unwrap();
        exchange(&a, &b);
        // Sin conexión, los dos cambian el saldo partiendo de 100.
        put(&a, "A", Policy::Strict, "caja", "saldo", "valor", Some("80")).unwrap();
        put(&b, "B", Policy::Strict, "caja", "saldo", "valor", Some("150")).unwrap();
        exchange(&a, &b);
        let (ra, rb) = (records(&a, "caja").unwrap(), records(&b, "caja").unwrap());
        assert_eq!(ra, rb, "los dos ven lo mismo");
        assert_eq!(ra[0].conflicts.len(), 1, "conflicto avisado, no mezclado");
        let mut both = vec![ra[0].fields[0].1.clone(), ra[0].conflicts[0].1[0].clone()];
        both.sort();
        assert_eq!(both, ["150", "80"], "se conservan los dos valores");

        // B corrige su propio valor sin haber visto el 80 de A (en el teléfono real pasó con un dígito
        // mal escrito): su corrección NO puede cerrar un conflicto que no vio.
        let (a2, b2) = (store("st2-a"), store("st2-b"));
        put(&a2, "A", Policy::Strict, "caja", "saldo", "valor", Some("100")).unwrap();
        exchange(&a2, &b2);
        put(&a2, "A", Policy::Strict, "caja", "saldo", "valor", Some("80")).unwrap();
        put(&b2, "B", Policy::Strict, "caja", "saldo", "valor", Some("1")).unwrap();
        put(&b2, "B", Policy::Strict, "caja", "saldo", "valor", Some("150")).unwrap();
        exchange(&a2, &b2);
        for s in [&a2, &b2] {
            let r = records(s, "caja").unwrap();
            assert_eq!(r[0].fields[0].1, "150");
            assert_eq!(r[0].conflicts, [("valor".to_string(), vec!["80".to_string()])], "el 80 sigue a la vista");
        }

        // Alguien elige (incluso otro valor) y la elección llega a todos.
        put(&b, "B", Policy::Strict, "caja", "saldo", "valor", Some("130")).unwrap();
        exchange(&a, &b);
        for s in [&a, &b] {
            let r = records(s, "caja").unwrap();
            assert_eq!(r[0].fields, [("valor".to_string(), "130".to_string())]);
            assert!(r[0].conflicts.is_empty());
        }
    }

    #[test]
    fn recibir_dos_veces_o_desordenado_da_lo_mismo() {
        let (a, b, c) = (store("idem-a"), store("idem-b"), store("idem-c"));
        for (i, v) in ["uno", "dos", "tres"].iter().enumerate() {
            put(&a, "A", Policy::Lww, "notas", "n1", "texto", Some(v)).unwrap();
            put(&a, "A", Policy::Lww, "notas", &format!("n{}", i + 2), "texto", Some(v)).unwrap();
        }
        let ops = a.ops_after(&Vector::new()).unwrap();
        assert_eq!(merge(&b, ops.clone()).unwrap(), 6);
        assert_eq!(merge(&b, ops.clone()).unwrap(), 0, "repetidas: no cambian nada");
        merge(&c, ops.into_iter().rev().collect()).unwrap();
        assert_eq!(records(&b, "notas").unwrap(), records(&c, "notas").unwrap());
        assert_eq!(get(&c, "notas", "n1", "texto").as_deref(), Some("tres"));
    }

    #[test]
    fn reloj_atrasado_no_rompe_el_orden() {
        let (a, b) = (store("hlc-a"), store("hlc-b"));
        // El reloj de A va una hora adelantado.
        a.set_meta("hlc", now_hlc() + (3_600_000 << 16)).unwrap();
        put(&a, "A", Policy::Lww, "perfil", "yo", "estado", Some("ocupado")).unwrap();
        exchange(&a, &b);
        // B cambia después de verlo: aunque su reloj vaya "antes", su cambio queda detrás.
        put(&b, "B", Policy::Lww, "perfil", "yo", "estado", Some("libre")).unwrap();
        exchange(&a, &b);
        assert_eq!(get(&a, "perfil", "yo", "estado").as_deref(), Some("libre"));
    }

    #[test]
    fn valores_guardados_cifrados() {
        let a = store("cifra");
        put(&a, "A", Policy::Lww, "perfil", "yo", "nombre", Some("texto-muy-secreto")).unwrap();
        let dir = std::env::temp_dir().join(format!("mm-sync-{}-cifra", std::process::id()));
        let raw = std::fs::read(dir.join("mesh.db")).unwrap();
        assert!(!raw.windows(17).any(|w| w == b"texto-muy-secreto"));
    }
}

// ---------- Consultas: filtrar, ordenar y limitar ----------
//
// Los valores están cifrados en la base de datos, así que no se puede filtrar con SQL: se descifra
// la colección y se filtra en memoria.
// ponytail: recorre toda la colección; añadir índices si alguna llega a decenas de miles de documentos.

/// Condición sobre un campo. `op`: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza`, `existe`.
/// `value` con la misma codificación con que se guardan los valores (p. ej. `[10]` o `["pan"]`).
#[derive(Debug, Clone)]
pub struct Filter {
    pub field: String,
    pub op: String,
    pub value: String,
}

/// Valor guardado → JSON (las apps lo guardan como `[valor]`; si no es JSON, es texto).
fn json(s: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(serde_json::Value::Array(mut a)) if a.len() == 1 => a.remove(0),
        Ok(v) => v,
        Err(_) => serde_json::Value::String(s.into()),
    }
}

/// Compara dos valores del mismo tipo (números, textos o booleanos); de tipos distintos, `None`.
fn compare(a: &serde_json::Value, b: &serde_json::Value) -> Option<std::cmp::Ordering> {
    use serde_json::Value::*;
    match (a, b) {
        (Number(x), Number(y)) => x.as_f64()?.partial_cmp(&y.as_f64()?),
        (String(x), String(y)) => Some(x.cmp(y)),
        (Bool(x), Bool(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn matches(r: &Record, f: &Filter) -> Result<bool> {
    use std::cmp::Ordering::*;
    let field = r.fields.iter().find(|(n, _)| *n == f.field).map(|(_, v)| json(v));
    if f.op == "existe" {
        return Ok(field.is_some());
    }
    let (Some(v), want) = (field, json(&f.value)) else { return Ok(false) };
    let text = |x: &serde_json::Value| x.as_str().map(str::to_lowercase);
    Ok(match f.op.as_str() {
        "==" => v == want,
        "!=" => v != want,
        "<" => compare(&v, &want) == Some(Less),
        "<=" => matches!(compare(&v, &want), Some(Less | Equal)),
        ">" => compare(&v, &want) == Some(Greater),
        ">=" => matches!(compare(&v, &want), Some(Greater | Equal)),
        "contiene" => matches!((text(&v), text(&want)), (Some(a), Some(b)) if a.contains(&b)),
        "empieza" => matches!((text(&v), text(&want)), (Some(a), Some(b)) if a.starts_with(&b)),
        op => return Err(format!("operador de consulta desconocido «{op}»").into()),
    })
}

/// Documentos de `collection` que cumplen todas las condiciones, ordenados por `order` (campo,
/// descendente) y como mucho `limit`. Los que no tienen el campo de orden van al final.
pub fn query(store: &Store, collection: &str, filters: &[Filter], order: Option<(&str, bool)>, limit: Option<usize>) -> Result<Vec<Record>> {
    let mut out = Vec::new();
    for r in records(store, collection)? {
        let mut ok = true;
        for f in filters {
            ok &= matches(&r, f)?;
        }
        if ok {
            out.push(r);
        }
    }
    if let Some((field, desc)) = order {
        let key = |r: &Record| r.fields.iter().find(|(n, _)| n == field).map(|(_, v)| json(v));
        out.sort_by(|a, b| match (key(a), key(b)) {
            (Some(x), Some(y)) => {
                let o = compare(&x, &y).unwrap_or(std::cmp::Ordering::Equal);
                if desc { o.reverse() } else { o }
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
    }
    if let Some(n) = limit {
        out.truncate(n);
    }
    Ok(out)
}
