//! Mediciones de velocidad reproducibles (no fallan: imprimen). Ejecutar en modo optimizado:
//!   cargo test --release --test velocidad -- --nocapture
//! Todo pasa por el núcleo de verdad: cifrado, SQLite en disco, Noise y ratchet.

use mobilemesh_core::{
    Identity, Store, deliver,
    identity::{Plain, to_hex},
    serve,
    sync::{self, Filter, Policy},
};
use std::{net::TcpListener, path::PathBuf, thread, time::Instant};

const N: usize = 1000;

fn device(name: &str) -> (PathBuf, Identity) {
    let dir = std::env::temp_dir().join(format!("mm-vel-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let id = Identity::create(&dir, &Plain).unwrap();
    (dir, id)
}

fn st(dir: &std::path::Path) -> Store {
    Store::open(dir, Identity::load(dir, &Plain).unwrap().db_key()).unwrap()
}

fn listen_once(dir: PathBuf) -> (String, thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let me = Identity::load(&dir, &Plain).unwrap();
        let _ = serve(&st(&dir), &me, l.accept().unwrap().0);
    });
    (addr, h)
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn velocidad() {
    let (ana, beto) = (device("ana"), device("beto"));
    let (a, b) = (ana.1.id(), beto.1.id());
    st(&ana.0).add_contact("beto", &b, None).unwrap();
    st(&beto.0).add_contact("ana", &a, None).unwrap();
    let store = st(&ana.0);
    let yo = to_hex(&ana.1.public);

    // 1. Escribir: N campos, uno a uno (cada uno cifrado y guardado en disco).
    let t = Instant::now();
    for i in 0..N {
        sync::put_as(&store, &yo, &a, Policy::Lww, "productos", &format!("p{i}"), "precio", Some(&format!("[{i}]"))).unwrap();
    }
    let escribir = ms(t);
    println!("MEDIDA escribir {N} campos: {escribir:.0} ms ({:.0} µs cada uno)", escribir * 1000.0 / N as f64);

    // 2. Leer la colección entera (descifrando).
    let t = Instant::now();
    let todos = sync::records(&store, "productos").unwrap();
    println!("MEDIDA leer {} registros: {:.1} ms", todos.len(), ms(t));

    // 3. Consultar: precio >= 500, los 20 más caros.
    let f = [Filter { field: "precio".into(), op: ">=".into(), value: "500".into() }];
    let t = Instant::now();
    let r = sync::query(&store, "productos", &f, Some(("precio", true)), Some(20)).unwrap();
    println!("MEDIDA consulta (filtro + orden + límite) sobre {N}: {:.1} ms → {} resultados", ms(t), r.len());

    // 4. Mensajes: 50 entregados en una conexión (Noise + ratchet + confirmación de guardado).
    // (50: por encima de 60 por minuto y contacto, el antispam los frena a propósito.)
    for i in 0..50 {
        store.queue(&b, &format!("mensaje {i}")).unwrap();
    }
    let (addr, h) = listen_once(beto.0.clone());
    let t = Instant::now();
    let n = deliver(&store, &ana.1, &b, &addr).unwrap();
    let enviar = ms(t);
    h.join().unwrap();
    println!("MEDIDA entregar {n} mensajes cifrados de extremo a extremo: {enviar:.0} ms ({:.1} ms cada uno, conexión incluida)", enviar / n as f64);

    // 5. Sincronizar: Beto recibe los N cambios de la colección compartida.
    store.set_share("productos", &b, Some(false)).unwrap();
    st(&beto.0).set_share("productos", &a, Some(true)).unwrap();
    let (addr, h) = listen_once(ana.0.clone());
    let t = Instant::now();
    let (recibidos, _, _) = mobilemesh_core::sync_shared(&st(&beto.0), &beto.1, &addr, &a).unwrap();
    let sincronizar = ms(t);
    h.join().unwrap();
    println!("MEDIDA sincronizar {recibidos} cambios a otro dispositivo: {sincronizar:.0} ms");
    assert_eq!(sync::records(&st(&beto.0), "productos").unwrap().len(), N);
}
