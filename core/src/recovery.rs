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

/// Código en papel: la clave de la cuenta tal cual, **sin contraseña**, para recuperar la cuenta
/// aunque se olvide la contraseña. 32 bytes + 3 de comprobación en base 32 (sin las letras que se
/// confunden: I, L, O, U) = 56 letras en 14 grupos de 4. Quien lo tenga entra en la cuenta: se
/// guarda como una llave (papel en casa), nunca en el teléfono ni en fotos.
const PAPEL: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn comprobacion(secret: &[u8]) -> [u8; 3] {
    use sha2::{Digest, Sha256};
    let h = Sha256::new_with_prefix(b"mobilemesh-papel-v1").chain_update(secret).finalize();
    [h[0], h[1], h[2]]
}

pub fn paper(id: &Identity) -> Result<String> {
    let secret = id.account_secret.ok_or("el código se crea desde el dispositivo principal de la cuenta")?;
    let mut bytes = secret.to_vec();
    bytes.extend_from_slice(&comprobacion(&secret));
    let (mut acc, mut bits, mut letras) = (0u64, 0, String::new());
    for b in bytes {
        acc = (acc << 8) | u64::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            letras.push(PAPEL[((acc >> bits) & 31) as usize] as char);
        }
        acc &= (1 << bits) - 1;
    }
    Ok(letras.as_bytes().chunks(4).map(|g| std::str::from_utf8(g).unwrap_or_default()).collect::<Vec<_>>().join("-"))
}

pub fn from_paper(code: &str) -> Result<Identity> {
    // Copiado a mano: sin espacios ni guiones, en mayúsculas, y O→0, I/L→1.
    let letras: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect();
    if letras.len() != 56 {
        return Err(format!("el código debe tener 56 letras y tiene {}", letras.len()).into());
    }
    let (mut acc, mut bits, mut bytes) = (0u64, 0, Vec::with_capacity(35));
    for c in letras.bytes() {
        let v = PAPEL.iter().position(|&p| p == c).ok_or_else(|| format!("letra no válida en el código: {}", c as char))?;
        acc = (acc << 5) | v as u64;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    let (secret, check) = bytes.split_at(32);
    if check != comprobacion(secret) {
        return Err("el código tiene algún error: revisa letra por letra".into());
    }
    Identity::from_account_secret(secret.try_into()?)
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
    fn codigo_en_papel() {
        let id = Identity::from_account_secret([5u8; 32]).unwrap();
        let code = paper(&id).unwrap();
        assert_eq!(code.len(), 56 + 13, "14 grupos de 4 con guiones: {code}");
        assert_eq!(from_paper(&code).unwrap().account, id.account);
        // Copiado a mano: minúsculas, espacios, O por 0 e I/L por 1.
        let mano = code.to_lowercase().replace('-', " ").replace('0', "o").replace('1', "l");
        assert_eq!(from_paper(&mano).unwrap().account, id.account);
        // Una letra cambiada o que falte: se detecta.
        let mut mal = code.clone().into_bytes();
        mal[0] = if mal[0] == b'A' { b'B' } else { b'A' };
        assert!(from_paper(&String::from_utf8(mal).unwrap()).is_err());
        assert!(from_paper(&code[..code.len() - 1]).is_err());
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
