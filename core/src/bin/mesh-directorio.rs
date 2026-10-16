//! `mesh-directorio`: crea la clave del proyecto y firma el directorio de relés y guardianes que
//! descargan las apps. La clave secreta nunca sale de este equipo: guárdala con copia aparte.
//!
//!   mesh-directorio clave <archivo>                         crea la clave (muestra la pública)
//!   mesh-directorio firmar <archivo> <versión> <relés> <guardianes>
//!       relés y guardianes separados por comas; la línea que imprime va al archivo del relé
//!       (`mesh-rele … --directorio <archivo>`).

use mobilemesh_core::{directorio::{Directorio, firmar}, identity::{from_hex, to_hex}};
use ed25519_dalek::SigningKey;
use std::{fs, process::exit};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let r = match args.as_slice() {
        ["clave", archivo] => clave(archivo),
        ["firmar", archivo, v, reles, guardianes] => firma(archivo, v, reles, guardianes),
        _ => Err("uso: mesh-directorio clave <archivo> | firmar <archivo> <versión> <relés> <guardianes>".into()),
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn clave(archivo: &str) -> mobilemesh_core::Result<()> {
    if fs::metadata(archivo).is_ok() {
        return Err("ese archivo ya existe; no se reemplaza una clave".into());
    }
    let mut secreto = [0u8; 32];
    getrandom::fill(&mut secreto).map_err(|e| e.to_string())?;
    fs::write(archivo, to_hex(&secreto))?;
    #[cfg(unix)]
    fs::set_permissions(archivo, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    println!("clave pública (va en directorio::CLAVE): {}", to_hex(&SigningKey::from_bytes(&secreto).verifying_key().to_bytes()));
    Ok(())
}

fn firma(archivo: &str, v: &str, reles: &str, guardianes: &str) -> mobilemesh_core::Result<()> {
    let secreto: [u8; 32] = from_hex(fs::read_to_string(archivo)?.trim())?.try_into().map_err(|_| "clave dañada")?;
    let lista = |s: &str| s.split(',').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect();
    let d = Directorio { v: v.parse()?, reles: lista(reles), guardianes: lista(guardianes) };
    println!("{}", firmar(&d, &secreto)?);
    Ok(())
}
