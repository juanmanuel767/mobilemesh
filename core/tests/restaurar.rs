//! Restaurar una cuenta nunca debe pisar una identidad que está bien solo porque el teléfono
//! esté bloqueado: al sobrescribir la clave se pierde también la clave de la base de datos.

use mobilemesh_core::{Identity, Result, identity::{Plain, Sealer}};

/// Simula un teléfono bloqueado: no puede abrir la clave, pero sí sellar.
struct Bloqueado;

impl Sealer for Bloqueado {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
        Ok(plain.to_vec())
    }
    fn open(&self, _sealed: &[u8]) -> Result<Vec<u8>> {
        Err("BLOQUEADO: desbloquea el teléfono".into())
    }
}

fn directorio(nombre: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mm-restaurar-{}-{nombre}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn con_el_telefono_bloqueado_no_se_reemplaza_la_identidad() {
    let dir = directorio("bloqueado");
    Identity::create(&dir, &Plain).unwrap();
    let antes = std::fs::read(dir.join("identity.key")).unwrap();

    assert!(Identity::replaceable(&dir, &Bloqueado).is_err());

    let despues = std::fs::read(dir.join("identity.key")).unwrap();
    assert_eq!(antes, despues, "identity.key no debe cambiar con el teléfono bloqueado");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sin_identidad_si_se_puede_reemplazar() {
    let dir = directorio("vacio");
    assert!(Identity::replaceable(&dir, &Plain).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
