//! Archivos (fotos, vídeos, documentos): se cifran por trozos con una clave propia de cada archivo.
//! La clave, el nombre y la huella SHA-256 viajan dentro del mensaje cifrado con el ratchet; los
//! trozos viajan aparte. Los dos teléfonos guardan el archivo cifrado y solo lo descifran al abrirlo.

use crate::{
    Result,
    identity::{from_hex_any, to_hex},
};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, Payload},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

/// Tamaño de cada trozo sin cifrar. Cifrado ocupa 16 bytes más (sello de integridad).
pub const CHUNK: usize = 32 * 1024;
const SEAL: usize = 16;
// ponytail: límite fijo; subirlo cuando haya que mover vídeos largos.
pub const MAX_SIZE: u64 = 200 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct FileMeta {
    pub name: String,
    pub mime: String,
    pub size: u64,
    /// Huella SHA-256 del archivo sin cifrar, en hexadecimal.
    pub sha256: String,
    /// Clave del archivo en hexadecimal (solo viaja dentro del mensaje cifrado de extremo a extremo).
    pub key: String,
}

/// Sin la clave: que nunca acabe en un registro por error (auditoría, Fase 13).
impl std::fmt::Debug for FileMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileMeta({}, {} bytes)", self.name, self.size)
    }
}

/// Número de trozos de un archivo de `size` bytes.
pub fn chunk_count(size: u64) -> u64 {
    size.div_ceil(CHUNK as u64).max(1)
}

/// Tamaño cifrado del trozo `index` de un archivo de `size` bytes.
fn sealed_len(size: u64, index: u64) -> usize {
    (size.saturating_sub(index * CHUNK as u64)).min(CHUNK as u64) as usize + SEAL
}

impl FileMeta {
    pub fn chunks(&self) -> u64 {
        chunk_count(self.size)
    }

    fn sealed_len(&self, index: u64) -> usize {
        sealed_len(self.size, index)
    }

    fn cipher(&self) -> Result<ChaCha20Poly1305> {
        let key: [u8; 32] = from_hex_any(&self.key)?.try_into().map_err(|_| "clave de archivo no válida")?;
        Ok(ChaCha20Poly1305::new(&key.into()))
    }
}

/// El número de trozo va en el nonce y en los datos autenticados: no se pueden reordenar ni mezclar.
fn nonce(index: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..8].copy_from_slice(&index.to_be_bytes());
    n
}

fn open_chunk(cipher: &ChaCha20Poly1305, meta: &FileMeta, index: u64, data: &[u8]) -> Result<Vec<u8>> {
    if index >= meta.chunks() || data.len() != meta.sealed_len(index) {
        return Err("trozo de archivo con tamaño o número incorrecto".into());
    }
    cipher
        .decrypt(&nonce(index).into(), Payload { msg: data, aad: &index.to_be_bytes() })
        .map_err(|_| "trozo de archivo alterado".into())
}

/// Cifra `src` en `dst` con una clave nueva y devuelve sus datos.
pub fn encrypt_file(src: &Path, dst: &Path, name: &str, mime: &str) -> Result<FileMeta> {
    encrypt_reader(&mut File::open(src)?, fs::metadata(src)?.len(), dst, name, mime)
}

/// Igual que `encrypt_file`, pero desde memoria: nunca toca el disco sin cifrar.
pub fn encrypt_bytes(data: &[u8], dst: &Path, name: &str, mime: &str) -> Result<FileMeta> {
    encrypt_reader(&mut &data[..], data.len() as u64, dst, name, mime)
}

fn encrypt_reader(input: &mut impl Read, size: u64, dst: &Path, name: &str, mime: &str) -> Result<FileMeta> {
    if size > MAX_SIZE {
        return Err(format!("el archivo pasa del límite de {} MB", MAX_SIZE / 1024 / 1024).into());
    }
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|e| e.to_string())?;
    let mut meta = FileMeta { name: name.into(), mime: mime.into(), size, sha256: String::new(), key: to_hex(&key) };
    let cipher = meta.cipher()?;
    let mut out = File::create(dst)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    for index in 0..meta.chunks() {
        let n = read_full(input, &mut buf)?;
        hash.update(&buf[..n]);
        let ct = cipher
            .encrypt(&nonce(index).into(), Payload { msg: &buf[..n], aad: &index.to_be_bytes() })
            .map_err(|_| "no se pudo cifrar el archivo")?;
        out.write_all(&ct)?;
    }
    out.sync_all()?;
    meta.sha256 = to_hex(&hash.finalize());
    Ok(meta)
}

fn read_full(f: &mut impl Read, buf: &mut [u8]) -> Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// Lee el trozo cifrado `index` del archivo guardado (para enviarlo tal cual).
pub fn read_chunk(blob: &Path, meta: &FileMeta, index: u64) -> Result<Vec<u8>> {
    read_sealed(blob, meta.size, index)
}

/// Lee un trozo cifrado sabiendo solo el tamaño (quien guarda una copia ajena no tiene la clave).
pub fn read_sealed(blob: &Path, size: u64, index: u64) -> Result<Vec<u8>> {
    let mut f = File::open(blob)?;
    f.seek(SeekFrom::Start(index * (CHUNK + SEAL) as u64))?;
    let mut data = vec![0u8; sealed_len(size, index)];
    f.read_exact(&mut data)?;
    Ok(data)
}

/// Cuántos trozos completos hay ya en una recepción a medias (para continuar desde ahí).
pub fn received_chunks(part: &Path) -> Result<u64> {
    let len = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let whole = len / (CHUNK + SEAL) as u64;
    // Un trozo a medias (corte de conexión) se descarta.
    if !len.is_multiple_of((CHUNK + SEAL) as u64) {
        File::options().write(true).open(part)?.set_len(whole * (CHUNK + SEAL) as u64)?;
    }
    Ok(whole)
}

/// Comprueba un trozo recibido y lo añade al final de la recepción a medias.
pub fn append_chunk(part: &Path, meta: &FileMeta, index: u64, data: &[u8]) -> Result<()> {
    if index != received_chunks(part)? {
        return Err("los trozos del archivo llegaron desordenados".into());
    }
    open_chunk(&meta.cipher()?, meta, index, data)?;
    let mut f = File::options().create(true).append(true).open(part)?;
    f.write_all(data)?;
    Ok(())
}

/// Añade un trozo sin poder descifrarlo (copias de otros): solo se comprueba orden y tamaño.
/// La integridad la comprueba el dueño al recuperarla, con la huella que solo él puede leer.
pub fn append_blind(part: &Path, size: u64, index: u64, data: &[u8]) -> Result<()> {
    if index != received_chunks(part)? || index >= chunk_count(size) || data.len() != sealed_len(size, index) {
        return Err("trozo de copia con orden o tamaño incorrecto".into());
    }
    let mut f = File::options().create(true).append(true).open(part)?;
    f.write_all(data)?;
    Ok(())
}

/// Descifra el archivo guardado. Si `dst` es `None`, solo comprueba la huella.
pub fn decrypt_file(blob: &Path, meta: &FileMeta, mut dst: Option<&mut dyn Write>) -> Result<()> {
    let cipher = meta.cipher()?;
    let mut hash = Sha256::new();
    for index in 0..meta.chunks() {
        let plain = open_chunk(&cipher, meta, index, &read_chunk(blob, meta, index)?)?;
        hash.update(&plain);
        if let Some(w) = dst.as_mut() {
            w.write_all(&plain)?;
        }
    }
    if to_hex(&hash.finalize()) != meta.sha256 {
        return Err("la huella del archivo no coincide: está incompleto o alterado".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cifrar_trocear_y_comprobar() {
        let dir = std::env::temp_dir().join(format!("mm-files-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        for size in [0usize, 1, CHUNK, CHUNK + 1, 3 * CHUNK + 777] {
            let data: Vec<u8> = (0..size).map(|i| (i * 31 % 251) as u8).collect();
            let (src, blob, part) = (dir.join("src"), dir.join("blob"), dir.join("part"));
            fs::write(&src, &data).unwrap();
            let _ = fs::remove_file(&part);
            let meta = encrypt_file(&src, &blob, "foto.jpg", "image/jpeg").unwrap();
            assert_eq!(meta.size, size as u64);
            // Recibir trozo a trozo, como por la red.
            for i in 0..meta.chunks() {
                append_chunk(&part, &meta, i, &read_chunk(&blob, &meta, i).unwrap()).unwrap();
            }
            let mut out = Vec::new();
            decrypt_file(&part, &meta, Some(&mut out)).unwrap();
            assert_eq!(out, data, "tamaño {size}");
            assert!(!fs::read(&blob).unwrap().windows(16).any(|w| size >= 16 && w == &data[..16]), "cifrado");
        }
    }
}
