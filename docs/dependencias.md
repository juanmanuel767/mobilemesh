# Dependencias y licencias

Revisado el 13-10-2026 con `cargo metadata` y `cargo audit`.

## Núcleo (Rust)

| Librería | Versión | Licencia | Para qué |
|---|---|---|---|
| snow | 0.10.0 | Apache-2.0 o MIT | Saludo y canal cifrado (Noise) |
| vodozemac | 0.11.1 | Apache-2.0 | Double Ratchet (Olm), auditada |
| ed25519-dalek | 2.2.0 | BSD-3-Clause | Firmas de cuenta, retirados, buzones |
| chacha20poly1305 | 0.10.1 | Apache-2.0 o MIT | Archivos, base de datos, copias |
| argon2 | 0.6.0 | MIT o Apache-2.0 | Copia de recuperación |
| sha2 | 0.10 | MIT o Apache-2.0 | Huellas |
| rusqlite (SQLite incluido) | 0.40.2 | MIT (SQLite: dominio público) | Base de datos local |
| serde, serde_json | 1.0 | MIT o Apache-2.0 | Formatos |
| base64 | 0.23.1 | MIT o Apache-2.0 | Codificación |
| getrandom | 0.3 | MIT o Apache-2.0 | Azar del sistema |
| uniffi | 0.32.2 | MPL-2.0 | Puente con Kotlin/Swift |

En total son 201 librerías, y **todas tienen licencias libres permisivas**: MIT, Apache-2.0, BSD,
ISC, Zlib, Unicode o MPL-2.0 (esta última solo exige publicar los cambios hechos a esos mismos
archivos). Ninguna es GPL. **`cargo audit` no encontró ninguna vulnerabilidad conocida** (contra
1307 avisos de la base de datos RustSec).

## Android

| Librería | Licencia | Dónde |
|---|---|---|
| JNA 5.17.0 | Apache-2.0 o LGPL-2.1 | Librería (`mobilemesh-sdk`) |
| zxing-android-embedded 4.3.0 | Apache-2.0 | Solo la app de pruebas (QR) |
