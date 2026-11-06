//! Pasar una base de datos de una versión anterior (sin cifrar) a la base
//! cifrada, sin perder ni un dato.
//!
//! Es la prueba que protege a quien ya tiene la app instalada: si la conversión
//! fallara, perdería sus mensajes y sus contactos.

use mobilemesh_core::{Identity, Store, identity::Plain, store::database_key_hex};
use std::path::{Path, PathBuf};

/// Abre la base de un dispositivo de prueba con su clave de cifrado.
fn open_con_clave(dir: &Path, key: &[u8; 32]) -> rusqlite::Connection {
    let db = rusqlite::Connection::open(dir.join("mesh.db")).unwrap();
    let hex = database_key_hex(key);
    db.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";")).unwrap();
    db
}

fn device(name: &str) -> (PathBuf, Identity) {
    let dir = std::env::temp_dir().join(format!("mm-mig-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = Identity::create(&dir, &Plain).unwrap();
    (dir, id)
}

/// Deja la base como la dejaría una versión anterior de la app: sin cifrar.
///
/// Es la operación inversa de la que hace la app al abrirla, y sirve para
/// fabricar el punto de partida de la prueba.
fn decrypt_to_plain(dir: &Path, key: &[u8; 32]) {
    let db_path = dir.join("mesh.db");
    let plain = dir.join("plano.db");
    let _ = std::fs::remove_file(&plain);
    {
        let db = rusqlite::Connection::open(&db_path).unwrap();
        let hex = database_key_hex(key);
        db.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";")).unwrap();
        let _ = db.query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()));
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        // KEY '' crea la copia sin cifrar.
        db.execute_batch(&format!(
            "ATTACH DATABASE '{}' AS plano KEY '';",
            plain.to_string_lossy()
        ))
        .unwrap();
        db.query_row("SELECT sqlcipher_export('plano')", [], |_| Ok(())).unwrap();
        db.execute_batch(&format!("PRAGMA plano.user_version = {version};")).unwrap();
        db.execute_batch("DETACH DATABASE plano;").unwrap();
    }
    for sufijo in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(dir.join(format!("mesh.db{sufijo}")));
    }
    std::fs::rename(&plain, &db_path).unwrap();
}

#[test]
fn la_base_sin_cifrar_de_una_version_anterior_se_convierte_sin_perder_nada() {
    let (alice, id) = device("alice");
    let (_, bob) = device("bob");
    let key = id.db_key();

    // 1. Datos guardados con la app, como los tendría cualquier usuario.
    let store = Store::open(&alice, key).unwrap();
    store.add_contact("bob", &bob.id(), None).unwrap();
    store.queue(&bob.id(), "mensaje-de-antes-de-la-actualizacion").unwrap();
    let antes = store.list().unwrap();
    let version_antes: i64 = {
        let db = open_con_clave(&alice, &key);
        db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
    };
    drop(store);
    assert_eq!(antes.len(), 1, "debería haber un mensaje guardado");

    // 2. La base se deja como la de la versión anterior: sin cifrar y legible.
    decrypt_to_plain(&alice, &key);
    let raw = std::fs::read(alice.join("mesh.db")).unwrap();
    assert!(raw.starts_with(b"SQLite format 3\0"), "no quedó en claro");
    assert!(
        raw.windows(4).any(|w| w == b"bob\0") || raw.windows(3).any(|w| w == b"bob"),
        "el nombre del contacto debería verse en claro en la base antigua"
    );

    // 3. Abrir la app convierte la base sola.
    let store = Store::open(&alice, key).unwrap();
    let despues = store.list().unwrap();

    // 4. No se perdió nada.
    assert_eq!(despues.len(), antes.len(), "se perdieron mensajes");
    assert_eq!(despues[0].body, antes[0].body, "el texto cambió");
    assert_eq!(despues[0].peer, antes[0].peer, "el contacto cambió");
    assert_eq!(store.contacts().unwrap().len(), 1, "se perdió el contacto");

    // 5. La versión del esquema se conservó: `sqlcipher_export` no la copia, y
    //    sin ella la app volvería a aplicar todas las migraciones.
    let db = open_con_clave(&alice, &key);
    let version_despues: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(version_despues, version_antes, "se perdió la versión del esquema");
    drop(db);

    // 6. Ahora el archivo está cifrado: ni el texto ni el nombre del contacto.
    let raw = std::fs::read(alice.join("mesh.db")).unwrap();
    assert!(!raw.starts_with(b"SQLite format 3\0"), "la cabecera sigue visible");
    assert!(
        !raw.windows(35).any(|w| w == b"mensaje-de-antes-de-la-actualizacion"),
        "el texto sigue en claro"
    );

    // 7. No quedó ninguna copia sin cifrar en el disco.
    for sobra in ["mesh.db.sin-cifrar", "mesh.db.cifrando", "plano.db"] {
        assert!(
            !alice.join(sobra).exists(),
            "quedó una copia sin cifrar: {sobra}"
        );
    }

    let _ = std::fs::remove_dir_all(&alice);
}

#[test]
fn una_base_que_no_es_base_da_un_error_claro_y_no_la_destruye() {
    let (dir, id) = device("corrupta");
    let basura = b"esto no es una base de datos de ningun tipo".to_vec();
    std::fs::write(dir.join("mesh.db"), &basura).unwrap();

    // `Store` no implementa `Debug` a propósito (no debe acabar en un registro),
    // así que el error se saca con un `match` en vez de con `unwrap_err`.
    let error = match Store::open(&dir, id.db_key()) {
        Ok(_) => panic!("una base de basura no debería abrirse"),
        Err(e) => e.to_string(),
    };
    assert!(
        error.contains("no se pudo abrir la base de datos"),
        "el error debería explicar qué pasó: {error}"
    );
    assert_eq!(
        std::fs::read(dir.join("mesh.db")).unwrap(),
        basura,
        "el archivo no debe tocarse si no se puede abrir"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
