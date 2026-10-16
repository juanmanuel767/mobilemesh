//! Copia de recuperación: la clave de la **cuenta** cifrada con una contraseña que elige el usuario.
//! Se guarda fuera del teléfono (papel, otro equipo, gestor de contraseñas). Sin la contraseña no sirve.

use crate::{Identity, Result, identity::{from_hex_any, to_hex}};
use argon2::Argon2;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, aead::{Aead, Payload}};

/// Versión del formato. Cambiarla si cambian los parámetros de Argon2 o el cifrado.
const PREFIX: &str = "MM3-";
const AAD: &[u8] = b"mobilemesh-recuperacion-v2";
pub const MIN_PASS: usize = 10;

/// Argon2id: cada intento de adivinar la contraseña cuesta memoria y tiempo. MM3 usa 64 MiB y 3
/// pasadas (MM2 usaba los mínimos de OWASP: 19 MiB y 2); la clave de cuenta no se puede cambiar,
/// así que conviene que la copia aguante mucho (auditoría, Fase 13).
pub(crate) fn key(pass: &str, salt: &[u8], strong: bool) -> Result<chacha20poly1305::Key> {
    let mut k = [0u8; 32];
    let argon = if strong {
        let params = argon2::Params::new(64 * 1024, 3, 1, Some(32)).map_err(|e| e.to_string())?;
        Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
    } else {
        Argon2::default()
    };
    argon.hash_password_into(pass.as_bytes(), salt, &mut k).map_err(|e| e.to_string())?;
    Ok(k.into())
}

// ponytail: código en hexadecimal (152 letras), cómodo en QR o archivo; añadir palabras (tipo BIP39) para papel.
pub fn export(id: &Identity, pass: &str) -> Result<String> {
    if pass.chars().count() < MIN_PASS {
        return Err(format!("la contraseña debe tener al menos {MIN_PASS} caracteres").into());
    }
    let secret = id.account_secret.ok_or("la copia se crea desde el dispositivo principal de la cuenta")?;
    let mut salt_nonce = [0u8; 28];
    getrandom::fill(&mut salt_nonce).map_err(|e| e.to_string())?;
    let (salt, nonce) = salt_nonce.split_at(16);
    let ct = ChaCha20Poly1305::new(&key(pass, salt, true)?)
        .encrypt(nonce.into(), Payload { msg: &secret, aad: AAD })
        .map_err(|_| "no se pudo cifrar la copia")?;
    Ok(format!("{PREFIX}{}{}", to_hex(&salt_nonce), to_hex(&ct)))
}

/// Recupera la cuenta y crea un dispositivo principal nuevo para ella.
pub fn import(code: &str, pass: &str) -> Result<Identity> {
    // Sin espacios ni guiones y en minúsculas: puede venir copiada a mano de un papel.
    let code: String = code.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>().to_lowercase();
    if code.starts_with("mm1") {
        return Err("esta copia es de una versión anterior (sin cuentas) y ya no sirve".into());
    }
    let (hex, strong) = match (code.strip_prefix("mm3"), code.strip_prefix("mm2")) {
        (Some(h), _) => (h, true),
        (_, Some(h)) => (h, false),
        _ => return Err("no es una copia de recuperación de MobileMesh".into()),
    };
    let b = from_hex_any(hex)?;
    if b.len() != 28 + 32 + 16 {
        return Err("la copia de recuperación está incompleta".into());
    }
    let (salt, rest) = b.split_at(16);
    let (nonce, ct) = rest.split_at(12);
    let secret = ChaCha20Poly1305::new(&key(pass, salt, strong)?)
        .decrypt(nonce.into(), Payload { msg: ct, aad: AAD })
        .map_err(|_| "contraseña incorrecta o copia alterada")?;
    Identity::from_account_secret(secret.as_slice().try_into()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_y_restauracion() {
        let id = Identity::from_account_secret([7u8; 32]).unwrap();
        let code = export(&id, "contraseña larga").unwrap();
        assert!(code.starts_with(PREFIX) && code.len() == 4 + 152);
        // Misma cuenta, pero un dispositivo nuevo (otra clave de dispositivo).
        let back = import(&code, "contraseña larga").unwrap();
        assert_eq!(back.account, id.account);
        assert_ne!(back.public, id.public);
        // Con espacios o saltos de línea (copiada a mano) también vale.
        let spaced = code.as_bytes().chunks(8).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join(" ");
        assert_eq!(import(&spaced, "contraseña larga").unwrap().account, id.account);
        assert!(import(&code, "otra contraseña").is_err());
        assert!(export(&id, "corta").is_err());
        // Cambiar un solo carácter invalida la copia.
        let mut bad = code.into_bytes();
        let last = bad.len() - 1;
        bad[last] = if bad[last] == b'0' { b'1' } else { b'0' };
        assert!(import(&String::from_utf8(bad).unwrap(), "contraseña larga").is_err());
    }

    #[test]
    fn las_copias_mm2_antiguas_siguen_sirviendo() {
        let id = Identity::from_account_secret([9u8; 32]).unwrap();
        // Copia hecha como antes (Argon2 con los parámetros mínimos y prefijo MM2).
        let salt_nonce = [3u8; 28];
        let (salt, nonce) = salt_nonce.split_at(16);
        let ct = ChaCha20Poly1305::new(&key("contraseña larga", salt, false).unwrap())
            .encrypt(nonce.into(), Payload { msg: &id.account_secret.unwrap(), aad: AAD })
            .unwrap();
        let old = format!("MM2-{}{}", to_hex(&salt_nonce), to_hex(&ct));
        assert_eq!(import(&old, "contraseña larga").unwrap().account, id.account);
        let new = export(&id, "contraseña larga").unwrap();
        assert!(new.starts_with("MM3-"), "las nuevas, más resistentes");
        assert_eq!(import(&new.to_uppercase(), "contraseña larga").unwrap().account, id.account, "copiada en mayúsculas");
    }
}
