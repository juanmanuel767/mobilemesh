//! Fase 13: mediciones de coste (ejecutar con `cargo test --release --test medidas -- --nocapture`).

use mobilemesh_core::{Identity, Store, deliver, identity::Plain, serve};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, atomic::{AtomicUsize, Ordering}},
    thread,
};

fn device(name: &str) -> (PathBuf, Identity) {
    let dir = std::env::temp_dir().join(format!("mm-med-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let id = Identity::create(&dir, &Plain).unwrap();
    (dir, id)
}

fn st(d: &(PathBuf, Identity)) -> Store {
    Store::open(&d.0, d.1.db_key()).unwrap()
}

/// Bob atiende una conexión pasando por un contador de bytes (en los dos sentidos).
fn counted(bob: &(PathBuf, Identity)) -> (String, Arc<AtomicUsize>, thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let front = TcpListener::bind("127.0.0.1:0").unwrap();
    let (addr, front_addr) = (l.local_addr().unwrap(), front.local_addr().unwrap().to_string());
    let (dir, me) = (bob.0.clone(), bob.1.clone());
    thread::spawn(move || {
        let _ = serve(&Store::open(&dir, me.db_key()).unwrap(), &me, l.accept().unwrap().0);
    });
    let bytes = Arc::new(AtomicUsize::new(0));
    let b2 = bytes.clone();
    let h = thread::spawn(move || {
        let a = front.accept().unwrap().0;
        let b = TcpStream::connect(addr).unwrap();
        let pipe = |mut from: TcpStream, mut to: TcpStream, n: Arc<AtomicUsize>| {
            thread::spawn(move || {
                let mut buf = [0u8; 16384];
                while let Ok(k @ 1..) = from.read(&mut buf) {
                    n.fetch_add(k, Ordering::Relaxed);
                    if to.write_all(&buf[..k]).is_err() {
                        break;
                    }
                }
                let _ = to.shutdown(std::net::Shutdown::Both);
            })
        };
        let x = pipe(a.try_clone().unwrap(), b.try_clone().unwrap(), b2.clone());
        let y = pipe(b, a, b2);
        let _ = (x.join(), y.join());
    });
    (front_addr, bytes, h)
}

/// Tamaño de la base tras pasar al archivo principal lo que está en el diario WAL.
fn db_size(dir: &std::path::Path) -> u64 {
    let db = rusqlite::Connection::open(dir.join("mesh.db")).unwrap();
    db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).unwrap();
    std::fs::metadata(dir.join("mesh.db")).unwrap().len()
}

#[test]
fn medidas() {
    let (alice, bob) = (device("alice"), device("bob"));
    st(&alice).add_contact("bob", &bob.1.id(), None).unwrap();
    st(&bob).add_contact("alice", &alice.1.id(), None).unwrap();
    let db_before = db_size(&bob.0);

    // 1) Primera conexión con un mensaje (incluye saludo y apertura del ratchet).
    let store = st(&alice);
    store.queue(&bob.1.id(), "hola, ¿qué tal?").unwrap();
    let (addr, bytes, h) = counted(&bob);
    deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap();
    h.join().unwrap();
    let first = bytes.load(Ordering::Relaxed);

    // 2) 50 mensajes cortos en una conexión (el límite es 60 por minuto).
    for i in 0..50 {
        store.queue(&bob.1.id(), &format!("mensaje corto número {i}")).unwrap();
    }
    let (addr, bytes, h) = counted(&bob);
    deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap();
    h.join().unwrap();
    let fifty = bytes.load(Ordering::Relaxed);
    let db_after = db_size(&bob.0);

    // 3) Un archivo de 1 MB.
    let src = alice.0.join("1mb.bin");
    std::fs::write(&src, (0..1_000_000u32).map(|i| (i % 251) as u8).collect::<Vec<_>>()).unwrap();
    mobilemesh_core::queue_file(&store, &bob.1.id(), &src, "1mb.bin", "application/octet-stream").unwrap();
    let (addr, bytes, h) = counted(&bob);
    deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap();
    h.join().unwrap();
    let file = bytes.load(Ordering::Relaxed);

    // 4) Sincronización sin cambios entre dos dispositivos de una cuenta.
    let tablet_dir = alice.0.with_extension("tableta");
    let _ = std::fs::remove_dir_all(&tablet_dir);
    let p = mobilemesh_core::Pending::create(&tablet_dir, &Plain).unwrap();
    let cert = mobilemesh_core::approve_link(&alice.1, &mobilemesh_core::link_request(&p)).unwrap();
    let tablet = (tablet_dir.clone(), mobilemesh_core::complete_link(p, &tablet_dir, &Plain, &cert).unwrap());
    let sync_once = || {
        let (addr, bytes, h) = counted(&tablet);
        mobilemesh_core::sync_with(&st(&alice), &alice.1, &addr).unwrap();
        h.join().unwrap();
        bytes.load(Ordering::Relaxed)
    };
    sync_once();
    let idle_sync = sync_once();
    println!("MEDIDA sincronización sin cambios: {idle_sync} bytes");

    println!("MEDIDA conexión + 1 mensaje (saludo, listas y ratchet): {first} bytes");
    println!("MEDIDA por mensaje corto (50 en una conexión): {} bytes", fifty / 50);
    println!("MEDIDA base de datos del receptor por mensaje: {} bytes", (db_after - db_before) / 51);
    println!("MEDIDA archivo de 1 000 000 bytes por la red: {file} bytes ({:.1} %)", file as f64 / 1e4);
    assert!(fifty / 50 < 2048, "un mensaje corto no debería pasar de 2 KB por la red");
    assert!((file as f64) < 1.5e6, "el archivo no debería ocupar más de un 50 % extra");
}
