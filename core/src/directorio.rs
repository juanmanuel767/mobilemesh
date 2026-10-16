//! Directorio (pieza aparte): lista **firmada** de relés y guardianes universales que las apps
//! descargan solas de su relé al arrancar. Así se añaden o quitan relés y guardianes sin publicar
//! una versión nueva de la app.
//!
//! - Lo firma el proyecto con su clave (Ed25519, `mesh-directorio firmar`); las apps llevan solo la
//!   clave pública (`CLAVE`). Un relé que lo sirva no puede meter direcciones falsas.
//! - Lleva un número de versión: solo se adopta uno más nuevo que el que ya se tiene.
//! - Sin directorio (o con uno roto) se usan las listas fijas `relay::UNIVERSALES` y
//!   `guardian::UNIVERSALES`.

use crate::{Result, Store, identity::{from_hex, from_hex_any, to_hex}};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Clave pública del proyecto con la que se comprueba el directorio.
// La secreta está solo en el PC del proyecto (~/.config/mobilemesh/directorio.clave).
pub const CLAVE: &str = "27f733499d6d96d5ceb024e8ede20d21bcb266cfcad15cc0d34b44f3cbcbf50d";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Directorio {
    pub v: u64,
    pub reles: Vec<String>,
    pub guardianes: Vec<String>,
}

const CONTEXTO: &[u8] = b"mobilemesh-directorio-v1\n";

/// Línea firmada: `base64(json).firma`.
pub fn firmar(d: &Directorio, secreto: &[u8; 32]) -> Result<String> {
    let json = serde_json::to_vec(d)?;
    let firma = SigningKey::from_bytes(secreto).sign(&[CONTEXTO, &json].concat());
    Ok(format!("{}.{}", B64.encode(&json), to_hex(&firma.to_bytes())))
}

pub fn verificar(linea: &str, clave: &str) -> Result<Directorio> {
    let (datos, firma) = linea.trim().split_once('.').ok_or("directorio mal formado")?;
    let json = B64.decode(datos).map_err(|_| "directorio mal codificado")?;
    let clave: [u8; 32] = from_hex(clave)?.try_into().map_err(|_| "clave del directorio no válida")?;
    let firma: [u8; 64] = from_hex_any(firma)?.try_into().map_err(|_| "firma del directorio no válida")?;
    VerifyingKey::from_bytes(&clave)
        .and_then(|k| k.verify_strict(&[CONTEXTO, &json].concat(), &Signature::from_bytes(&firma)))
        .map_err(|_| "la firma del directorio no es del proyecto")?;
    Ok(serde_json::from_slice(&json)?)
}

/// El directorio guardado en este dispositivo (comprobado otra vez al leerlo).
pub fn guardado(store: &Store) -> Option<Directorio> {
    store.setting("directorio").ok().flatten().and_then(|l| verificar(&l, CLAVE).ok())
}

pub fn reles(store: &Store) -> Vec<String> {
    match guardado(store) {
        Some(d) if !d.reles.is_empty() => d.reles,
        _ => crate::relay::UNIVERSALES.iter().map(|r| r.to_string()).collect(),
    }
}

pub fn guardianes(store: &Store) -> Vec<String> {
    match guardado(store) {
        Some(d) if !d.guardianes.is_empty() => d.guardianes,
        _ => crate::guardian::UNIVERSALES.iter().map(|g| g.to_string()).collect(),
    }
}

/// Pide el directorio a cada relé hasta que uno dé uno válido; devuelve el más nuevo (y su línea).
pub fn descargar(reles: &[String]) -> Option<(Directorio, String)> {
    descargar_con(reles, CLAVE)
}

/// Como `descargar`, comprobando con otra clave (pruebas).
pub fn descargar_con(reles: &[String], clave: &str) -> Option<(Directorio, String)> {
    reles
        .iter()
        .filter_map(|r| crate::relay::directory(r).ok())
        .filter_map(|l| verificar(&l, clave).ok().map(|d| (d, l)))
        .max_by_key(|(d, _)| d.v)
}

/// Busca un directorio más nuevo en mis relés (y en los universales) y lo guarda. `true` si cambió.
pub fn actualizar(store: &Store) -> Result<bool> {
    let mut donde = store.relays()?;
    donde.extend(reles(store));
    donde.dedup();
    let actual = guardado(store).map_or(0, |d| d.v);
    match descargar(&donde) {
        Some((d, linea)) if d.v > actual => {
            store.set_setting("directorio", Some(&linea))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
