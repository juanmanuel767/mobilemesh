//! Lo que se le exige a una base de datos: que no se corrompa si el proceso
//! muere escribiendo, que dos escritores a la vez no se pisen y que un archivo
//! alterado se note en vez de leerse como si fuera bueno.
//!
//! Los procesos hijos son el propio binario de pruebas relanzado con variables
//! de entorno: así el hijo usa la librería de verdad (SQLCipher incluido).

use mobilemesh_core::{Identity, Store, identity::Plain, store::database_key_hex};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const PAR: &str = "peer-durabilidad";

fn device(name: &str) -> (PathBuf, Identity) {
    let dir = std::env::temp_dir().join(format!("mm-dur-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let id = Identity::create(&dir, &Plain).unwrap();
    (dir, id)
}

fn open_con_clave(dir: &Path, key: &[u8; 32]) -> rusqlite::Connection {
    let db = rusqlite::Connection::open(dir.join("mesh.db")).unwrap();
    let hex = database_key_hex(key);
    db.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";")).unwrap();
    db
}

fn valor(id: &str) -> String {
    format!("valor-confirmado-de-{id}")
}

fn from_hex(s: &str) -> [u8; 32] {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    k
}

fn to_hex(k: &[u8; 32]) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

/// Relanza este mismo binario ejecutando solo la prueba `prueba` como hijo.
fn lanzar_hijo(prueba: &str, dir: &Path, key: &[u8; 32], prefijo: &str, n: u32) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([prueba, "--exact", "--nocapture", "--test-threads=1"])
        .env("MM_HIJO_DIR", dir)
        .env("MM_HIJO_KEY", to_hex(key))
        .env("MM_HIJO_PREFIJO", prefijo)
        .env("MM_HIJO_N", n.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn esperar_archivo(p: &Path, hijo: &mut Child) {
    let limite = Instant::now() + Duration::from_secs(60);
    while !p.exists() {
        if let Some(st) = hijo.try_wait().unwrap() {
            panic!("el hijo terminó ({st}) antes de crear {}", p.display());
        }
        assert!(Instant::now() < limite, "el hijo no creó {} en 60 s", p.display());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn hijo_datos() -> Option<(PathBuf, [u8; 32], String, u32)> {
    let dir = std::env::var("MM_HIJO_DIR").ok()?;
    let key = from_hex(&std::env::var("MM_HIJO_KEY").unwrap());
    let prefijo = std::env::var("MM_HIJO_PREFIJO").unwrap();
    let n = std::env::var("MM_HIJO_N").unwrap().parse().unwrap();
    Some((PathBuf::from(dir), key, prefijo, n))
}

/// Anota en un archivo (y lo fuerza a disco) un registro ya confirmado.
fn anotar(f: &mut std::fs::File, linea: &str) {
    writeln!(f, "{linea}").unwrap();
    f.sync_all().unwrap();
}

#[test]
fn caida_a_media_escritura_no_corrompe_la_base() {
    // Hijo: abre la base y escribe sin parar hasta que lo matan.
    if let Some((dir, key, prefijo, _)) = hijo_datos() {
        let store = Store::open(&dir, key).unwrap();
        let mut i = 0u64;
        loop {
            let id = format!("{prefijo}-{i}");
            store.save_incoming(PAR, &id, &valor(&id)).unwrap();
            if i == 0 {
                std::fs::write(dir.join("listo-hijo"), "1").unwrap();
            }
            i += 1;
        }
    }

    // Lo que se protege: si el proceso muere (batería, el sistema lo mata) en
    // plena escritura, el usuario conserva lo que ya estaba guardado. No hay
    // copia en ningún servidor.
    let (dir, id) = device("caida");
    let key = id.db_key();
    let confirmados: Vec<String> = (0..20).map(|i| format!("base-{i}")).collect();
    {
        let store = Store::open(&dir, key).unwrap();
        for c in &confirmados {
            store.save_incoming(PAR, c, &valor(c)).unwrap();
        }
    }

    let mut hijo = lanzar_hijo("caida_a_media_escritura_no_corrompe_la_base", &dir, &key, "hijo", 0);
    esperar_archivo(&dir.join("listo-hijo"), &mut hijo);
    // Margen para que esté de lleno en el bucle, sin depender de un tiempo justo.
    std::thread::sleep(Duration::from_millis(700));
    assert!(hijo.try_wait().unwrap().is_none(), "el hijo murió solo, no lo matamos nosotros");
    hijo.kill().unwrap(); // SIGKILL en Unix
    hijo.wait().unwrap();

    // a) Se vuelve a abrir sin error.
    let store = Store::open(&dir, key).expect("la base no se pudo reabrir tras la caída");

    // b) Todo lo confirmado sigue ahí con su valor.
    let filas = store.list().expect("no se pudo leer tras la caída");
    for c in &confirmados {
        let fila = filas.iter().find(|f| &f.id == c).unwrap_or_else(|| panic!("se perdió el registro confirmado {c}"));
        assert_eq!(fila.body, valor(c), "el registro {c} cambió de valor");
    }
    // Lo que escribió el hijo y sí llegó a confirmarse debe estar entero, nunca a medias.
    for f in filas.iter().filter(|f| f.id.starts_with("hijo-")) {
        assert_eq!(f.body, valor(&f.id), "registro del hijo a medias: {}", f.id);
    }
    drop(store);

    // c) La base está íntegra.
    let db = open_con_clave(&dir, &key);
    let r: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(r, "ok", "PRAGMA integrity_check tras SIGKILL");
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dos_escritores_a_la_vez_no_se_pisan() {
    // Hijo: abre, avisa, espera la señal de salida común y escribe `n` registros.
    if let Some((dir, key, prefijo, n)) = hijo_datos() {
        let store = Store::open(&dir, key).unwrap();
        let mut ok = std::fs::File::create(dir.join(format!("ok-{prefijo}"))).unwrap();
        let mut bloqueos = std::fs::File::create(dir.join(format!("err-{prefijo}"))).unwrap();
        std::fs::write(dir.join(format!("listo-{prefijo}")), "1").unwrap();
        while !dir.join("ya").exists() {
            std::thread::sleep(Duration::from_millis(2));
        }
        for i in 0..n {
            let id = format!("{prefijo}-{i}");
            match store.save_incoming(PAR, &id, &valor(&id)) {
                Ok(()) => anotar(&mut ok, &id),
                // Error limpio: se anota y se sigue.
                Err(e) => anotar(&mut bloqueos, &format!("{e}").replace('\n', " ")),
            }
        }
        std::process::exit(0);
    }

    // Lo que se protege: la app y su servicio escriben a la vez en la misma
    // base; puede haber espera o un error limpio, nunca datos pisados o a medias.
    // Se usan dos PROCESOS (no hilos): es el uso real, cada uno con su conexión.
    let (dir, id) = device("dos");
    let key = id.db_key();
    drop(Store::open(&dir, key).unwrap()); // crea el esquema y activa WAL

    const N: u32 = 300;
    let prueba = "dos_escritores_a_la_vez_no_se_pisan";
    let mut a = lanzar_hijo(prueba, &dir, &key, "a", N);
    let mut b = lanzar_hijo(prueba, &dir, &key, "b", N);
    esperar_archivo(&dir.join("listo-a"), &mut a);
    esperar_archivo(&dir.join("listo-b"), &mut b);
    std::fs::write(dir.join("ya"), "1").unwrap();
    assert!(a.wait().unwrap().success(), "el escritor a falló (panic o error no limpio)");
    assert!(b.wait().unwrap().success(), "el escritor b falló (panic o error no limpio)");

    let store = Store::open(&dir, key).expect("no se pudo reabrir tras los dos escritores");
    let filas = store.list().unwrap();
    let mut confirmados = 0;
    let mut errores = Vec::new();
    for p in ["a", "b"] {
        for l in std::fs::read_to_string(dir.join(format!("ok-{p}"))).unwrap().lines() {
            let f = filas.iter().find(|f| f.id == l).unwrap_or_else(|| panic!("se perdió {l}, que se confirmó"));
            assert_eq!(f.body, valor(l), "registro {l} pisado o a medias");
            confirmados += 1;
        }
        errores.extend(std::fs::read_to_string(dir.join(format!("err-{p}"))).unwrap().lines().map(String::from));
    }
    // Un error aceptable es solo «base ocupada»; cualquier otro es un fallo real.
    for e in &errores {
        assert!(e.contains("locked") || e.contains("busy"), "error que no es de bloqueo: {e}");
    }
    // Nada que no se haya confirmado (ni de más ni a medias).
    assert_eq!(filas.len(), confirmados, "hay registros que ningún escritor confirmó");
    println!("escritores: {confirmados} confirmados de {}, {} errores de bloqueo", 2 * N, errores.len());
    drop(store);

    let db = open_con_clave(&dir, &key);
    let r: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(r, "ok", "PRAGMA integrity_check tras dos escritores");
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn base_alterada_falla_limpio_y_no_devuelve_basura() {
    // Lo que se protege: que una corrupción (disco defectuoso, copia a medias)
    // se NOTE, en vez de leerse como datos válidos. Un dato equivocado dado por
    // bueno es peor que un error.
    let (dir, id) = device("alterada");
    let key = id.db_key();
    let originales: Vec<(String, String)> = (0..300).map(|i| (format!("reg-{i}"), format!("texto-original-{i}-{}", "x".repeat(60)))).collect();
    {
        let store = Store::open(&dir, key).unwrap();
        for (i, v) in &originales {
            store.save_incoming(PAR, i, v).unwrap();
        }
    }
    // Todo al archivo principal, sin restos en el -wal.
    {
        let db = open_con_clave(&dir, &key);
        db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
    }
    for s in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(dir.join(format!("mesh.db{s}")));
    }

    // Se sobrescriben 64 bytes a mitad del archivo (lejos del encabezado).
    let ruta = dir.join("mesh.db");
    let mut bytes = std::fs::read(&ruta).unwrap();
    assert!(bytes.len() > 16 * 4096, "la base es demasiado pequeña para el ensayo: {}", bytes.len());
    let pos = bytes.len() / 2;
    for b in &mut bytes[pos..pos + 64] {
        *b = !*b;
    }
    std::fs::write(&ruta, &bytes).unwrap();

    // Abrir y leer: o hay un Err, o cada dato devuelto es EXACTAMENTE el original.
    let mut fallo_visible = false;
    match Store::open(&dir, key) {
        Err(e) => {
            println!("al abrir: error limpio: {e}");
            assert!(!format!("{e}").is_empty(), "el error no tiene mensaje");
            fallo_visible = true;
        }
        Ok(store) => match store.list() {
            Err(e) => {
                println!("al leer: error limpio: {e}");
                assert!(!format!("{e}").is_empty(), "el error no tiene mensaje");
                fallo_visible = true;
            }
            Ok(filas) => {
                for f in &filas {
                    let (_, v) = originales.iter().find(|(i, _)| *i == f.id).unwrap_or_else(|| panic!("registro inventado: {}", f.id));
                    assert_eq!(&f.body, v, "dato equivocado devuelto como bueno: {}", f.id);
                }
            }
        },
    }

    // Aunque el almacén no hubiera tropezado, la base debe delatar la alteración.
    let db = open_con_clave(&dir, &key);
    let r: rusqlite::Result<String> = db.query_row("PRAGMA integrity_check", [], |r| r.get(0));
    match r {
        Err(e) => println!("integrity_check: error limpio: {e}"),
        Ok(s) => {
            println!("integrity_check: {s}");
            assert_ne!(s, "ok", "la base alterada pasó la comprobación de integridad");
        }
    }
    println!("el almacén notó la alteración al abrir/leer: {fallo_visible}");
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn comprobacion_de_integridad_distingue_sana_de_danada() {
    // Lo que se protege: que la app pueda preguntar «¿está sana mi base?» y que una
    // corrupción que el almacén no tocó al abrir/leer igualmente se señale.
    let (dir, id) = device("integridad");
    let key = id.db_key();
    {
        let store = Store::open(&dir, key).unwrap();
        for i in 0..300 {
            store.save_incoming(PAR, &format!("reg-{i}"), &format!("texto-{i}-{}", "x".repeat(60))).unwrap();
        }
        // a) Sana: la comprobación no encuentra problemas.
        let problemas = store.integrity_check().expect("la comprobación falló en una base sana");
        assert!(problemas.is_empty(), "base sana señalada como dañada: {problemas:?}");
    }
    {
        let db = open_con_clave(&dir, &key);
        db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
    }
    for s in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(dir.join(format!("mesh.db{s}")));
    }

    // b) Se sobrescriben 64 bytes a mitad del archivo.
    let ruta = dir.join("mesh.db");
    let mut bytes = std::fs::read(&ruta).unwrap();
    assert!(bytes.len() > 16 * 4096, "la base es demasiado pequeña para el ensayo: {}", bytes.len());
    let pos = bytes.len() / 2;
    for b in &mut bytes[pos..pos + 64] {
        *b = !*b;
    }
    std::fs::write(&ruta, &bytes).unwrap();

    // Debe señalar el daño: con problemas listados o con un error claro, nunca «sana».
    match Store::open(&dir, key) {
        Err(e) => println!("al abrir: error limpio: {e}"),
        Ok(store) => match store.integrity_check() {
            Err(e) => println!("integrity_check: error limpio: {e}"),
            Ok(problemas) => {
                println!("integrity_check: {problemas:?}");
                assert!(!problemas.is_empty(), "la base alterada se dio por sana");
            }
        },
    }
    let _ = std::fs::remove_dir_all(&dir);
}
