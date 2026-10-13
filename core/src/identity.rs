//! Identidad: una **cuenta** (clave de firma Ed25519) y **este dispositivo** (clave X25519 de Noise).
//! La cuenta firma la clave de cada dispositivo suyo (certificado). Los contactos confían en la
//! cuenta, así que aceptan cualquier dispositivo vinculado a ella, y ninguna clave sale nunca del suyo.

use crate::Result;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use std::{
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

/// Saludo Noise: autenticación mutua por clave, cifrado ChaCha20-Poly1305 y claves nuevas en cada sesión.
pub const NOISE: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const FILE: &str = "identity.key";
const CERT_CONTEXT: &[u8] = b"mobilemesh-dispositivo-v1";
/// Formato del archivo: [versión, tipo] + datos. Tipos: 0 esperando vinculación, 1 vinculado, 2 principal.
const VERSION: u8 = 2;

#[derive(Clone)]
pub struct Identity {
    /// Clave X25519 de este dispositivo (Noise).
    pub private: Vec<u8>,
    pub public: Vec<u8>,
    /// Clave pública de la cuenta: lo que conocen los contactos.
    pub account: [u8; 32],
    /// Firma de la cuenta sobre la clave de este dispositivo.
    pub cert: [u8; 64],
    /// Solo en el dispositivo principal: permite vincular otros y crear la copia de recuperación.
    pub account_secret: Option<[u8; 32]>,
}

/// Dispositivo recién instalado que espera el certificado de su cuenta.
pub struct Pending {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

/// Lo que hay guardado en un dispositivo.
pub enum Stored {
    Pending(Pending),
    Ready(Identity),
}

/// Protege la clave en disco. Android la cifra con Keystore; en PC se usa `Plain` (archivo 0600).
pub trait Sealer {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>>;
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>>;
}

pub struct Plain;

impl Sealer for Plain {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
        Ok(plain.to_vec())
    }
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>> {
        Ok(sealed.to_vec())
    }
}

/// Prefijo con el que la plataforma avisa de que no puede abrir la clave porque el teléfono está
/// bloqueado (no porque esté dañada). Nunca hay que ofrecer borrar nada en ese caso.
pub const LOCKED: &str = "BLOQUEADO";

const DAMAGED: &str = "la identidad de este dispositivo está dañada; hay que restaurarla desde la copia de recuperación";

fn device_keys() -> Result<(Vec<u8>, Vec<u8>)> {
    let kp = snow::Builder::new(NOISE.parse()?).generate_keypair()?;
    Ok((kp.private, kp.public))
}

fn random32() -> Result<[u8; 32]> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| e.to_string())?;
    Ok(b)
}

fn cert_message(device_public: &[u8]) -> Vec<u8> {
    [CERT_CONTEXT, device_public].concat()
}

/// Comprueba que `account` firmó la clave de dispositivo `device_public`.
pub fn verify_cert(account: &[u8], device_public: &[u8], cert: &[u8]) -> Result<()> {
    let account: &[u8; 32] = account.try_into().map_err(|_| "clave de cuenta con tamaño incorrecto")?;
    let cert: &[u8; 64] = cert.try_into().map_err(|_| "certificado con tamaño incorrecto")?;
    VerifyingKey::from_bytes(account)
        .and_then(|k| k.verify_strict(&cert_message(device_public), &Signature::from_bytes(cert)))
        .map_err(|_| "el certificado del dispositivo no es válido".into())
}

impl Identity {
    /// Cuenta nueva con este dispositivo como principal.
    pub fn create(dir: &Path, s: &dyn Sealer) -> Result<Identity> {
        if exists(dir) {
            return Err("no se pudo crear la identidad: ya existe una".into());
        }
        let id = Self::from_account_secret(random32()?)?;
        id.save(dir, s)?;
        Ok(id)
    }

    /// Dispositivo principal nuevo para una cuenta existente (al restaurar desde la copia).
    pub fn from_account_secret(secret: [u8; 32]) -> Result<Identity> {
        let (private, public) = device_keys()?;
        Self::primary(secret, private, public)
    }

    fn primary(secret: [u8; 32], private: Vec<u8>, public: Vec<u8>) -> Result<Identity> {
        let key = SigningKey::from_bytes(&secret);
        let cert = key.sign(&cert_message(&public)).to_bytes();
        Ok(Identity { private, public, account: key.verifying_key().to_bytes(), cert, account_secret: Some(secret) })
    }

    /// Firma la clave de otro dispositivo para vincularlo a esta cuenta.
    pub fn approve(&self, device_public: &[u8]) -> Result<[u8; 64]> {
        let secret = self.account_secret.ok_or("solo el dispositivo principal de la cuenta puede vincular otros")?;
        if device_public.len() != 32 {
            return Err("clave de dispositivo con tamaño incorrecto".into());
        }
        Ok(SigningKey::from_bytes(&secret).sign(&cert_message(device_public)).to_bytes())
    }

    /// Prueba que se envía en el saludo: «esta cuenta firmó la clave con la que hablo».
    pub fn proof(&self) -> Vec<u8> {
        [self.account.as_slice(), &self.cert].concat()
    }

    pub fn save(&self, dir: &Path, s: &dyn Sealer) -> Result<()> {
        let mut b = vec![VERSION, if self.account_secret.is_some() { 2 } else { 1 }];
        b.extend([self.private.as_slice(), &self.public, &self.account, &self.cert].concat());
        if let Some(secret) = &self.account_secret {
            b.extend(secret);
        }
        write_secure(dir, &s.seal(&b)?)
    }

    pub fn load(dir: &Path, s: &dyn Sealer) -> Result<Identity> {
        match load_any(dir, s)? {
            Stored::Ready(id) => Ok(id),
            Stored::Pending(_) => Err("este dispositivo todavía espera la vinculación con su cuenta".into()),
        }
    }

    /// Clave para cifrar la base de datos local. Sale de la clave de este dispositivo, así que
    /// queda protegida igual que ella (Keystore) y no hace falta guardarla aparte.
    pub fn db_key(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        Sha256::new_with_prefix(b"mobilemesh-bd-v1").chain_update(&self.private).finalize().into()
    }

    /// Identificador público de la cuenta, para compartir con los contactos.
    pub fn id(&self) -> String {
        to_hex(&self.account)
    }
}

impl Pending {
    pub fn create(dir: &Path, s: &dyn Sealer) -> Result<Pending> {
        if exists(dir) {
            return Err("este dispositivo ya tiene una identidad".into());
        }
        let (private, public) = device_keys()?;
        write_secure(dir, &s.seal(&[&[VERSION, 0], private.as_slice(), &public].concat())?)?;
        Ok(Pending { private, public })
    }

    /// Instala el certificado recibido del dispositivo principal, tras comprobarlo.
    pub fn complete(self, dir: &Path, s: &dyn Sealer, account: &[u8], cert: &[u8]) -> Result<Identity> {
        verify_cert(account, &self.public, cert)?;
        let id = Identity {
            private: self.private,
            public: self.public,
            account: account.try_into()?,
            cert: cert.try_into()?,
            account_secret: None,
        };
        id.save(dir, s)?;
        Ok(id)
    }
}

pub fn exists(dir: &Path) -> bool {
    dir.join(FILE).exists()
}

/// Escribe de forma segura ante apagones: archivo aparte, forzado al disco y renombrado.
/// (Pasó en un teléfono real sin batería; Android prohíbe hard_link en la carpeta de la app.)
fn write_secure(dir: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(&tmp, dir.join(FILE))?;
    fs::File::open(dir)?.sync_all()?; // grabar también la entrada del directorio
    Ok(())
}

pub fn load_any(dir: &Path, s: &dyn Sealer) -> Result<Stored> {
    let raw = fs::read(dir.join(FILE)).map_err(|e| format!("sin identidad en {}: {e}", dir.display()))?;
    // Antes de la Fase 2 la clave estaba sin cifrar (64 bytes): se acepta y se protege al guardarla.
    let b = match s.open(&raw) {
        Ok(b) => b,
        Err(_) if raw.len() == 64 => raw,
        // Teléfono bloqueado: la clave está bien, solo hay que esperar a que se desbloquee.
        Err(e) if e.to_string().starts_with(LOCKED) => return Err(e),
        Err(e) => return Err(format!("{DAMAGED} ({e})").into()),
    };
    let device_ok = |k: &[u8]| k.len() >= 64 && public_of(&k[..32]) == k[32..64];
    match (b.first(), b.get(1), b.len()) {
        // Formato anterior a las cuentas: solo clave de dispositivo. Se convierte en principal de una cuenta nueva.
        (_, _, 64) if device_ok(&b) => {
            let id = Identity::primary(random32()?, b[..32].to_vec(), b[32..].to_vec())?;
            id.save(dir, s)?;
            Ok(Stored::Ready(id))
        }
        (Some(&VERSION), Some(0), 66) if device_ok(&b[2..]) => {
            Ok(Stored::Pending(Pending { private: b[2..34].to_vec(), public: b[34..66].to_vec() }))
        }
        (Some(&VERSION), Some(kind @ (1 | 2)), len) if len == 162 + 32 * (*kind as usize - 1) && device_ok(&b[2..]) => {
            let id = Identity {
                private: b[2..34].to_vec(),
                public: b[34..66].to_vec(),
                account: b[66..98].try_into()?,
                cert: b[98..162].try_into()?,
                account_secret: if *kind == 2 { Some(b[162..194].try_into()?) } else { None },
            };
            verify_cert(&id.account, &id.public, &id.cert).map_err(|_| DAMAGED)?;
            if let Some(secret) = id.account_secret
                && SigningKey::from_bytes(&secret).verifying_key().to_bytes() != id.account
            {
                return Err(DAMAGED.into());
            }
            Ok(Stored::Ready(id))
        }
        _ => Err(DAMAGED.into()),
    }
}

/// Clave pública que corresponde a una privada X25519.
fn public_of(private: &[u8]) -> Vec<u8> {
    use snow::{params::DHChoice, resolvers::{CryptoResolver, DefaultResolver}};
    let mut dh = DefaultResolver.resolve_dh(&DHChoice::Curve25519).expect("X25519 siempre disponible");
    dh.set(private);
    dh.pubkey().to_vec()
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Lee una clave pública de 32 bytes escrita en hexadecimal.
pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    if s.len() != 64 {
        return Err("la clave debe tener 64 caracteres hexadecimales".into());
    }
    from_hex_any(s)
}

pub fn from_hex_any(s: &str) -> Result<Vec<u8>> {
    // Solo minúsculas: una misma clave escrita de dos maneras podría saltarse comparaciones.
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err("texto hexadecimal no válido".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "clave con caracteres no válidos".into()))
        .collect()
}
