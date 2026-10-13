//! Pruebas de la Fase 4/5 con dos o tres "teléfonos" simulados en el mismo equipo.

use mobilemesh_core::{Identity, Store, deliver, identity::Plain, serve};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

fn device(name: &str) -> (PathBuf, Identity) {
    let dir = std::env::temp_dir().join(format!("mm-{}-{name}-{:?}", std::process::id(), thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let id = Identity::create(&dir, &Plain).unwrap();
    (dir, id)
}

/// Abre la base de datos de un dispositivo de prueba con la clave de su identidad.
fn st(dir: &std::path::Path) -> Store {
    Store::open(dir, Identity::load(dir, &Plain).unwrap().db_key()).unwrap()
}

fn befriend(a: &(PathBuf, Identity), an: &str, b: &(PathBuf, Identity), bn: &str) {
    st(&a.0).add_contact(bn, &b.1.id(), None).unwrap();
    st(&b.0).add_contact(an, &a.1.id(), None).unwrap();
}

/// Arranca un dispositivo que atiende UNA conexión; devuelve su dirección y el resultado.
fn listen_once(dir: PathBuf) -> (String, thread::JoinHandle<Result<usize, String>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let me = Identity::load(&dir, &Plain).unwrap();
        let store = st(&dir);
        serve(&store, &me, l.accept().unwrap().0).map_err(|e| e.to_string())
    });
    (addr, h)
}

fn statuses(dir: &std::path::Path) -> Vec<String> {
    st(dir).list().unwrap().into_iter().map(|r| format!("{}:{}", r.status, r.body)).collect()
}

#[test]
fn entrega_cifrada_con_confirmacion() {
    let (alice, bob) = (device("alice"), device("bob"));
    befriend(&alice, "alice", &bob, "bob");
    let (addr, h) = listen_once(bob.0.clone());

    let store = st(&alice.0);
    store.queue(&bob.1.id(), "hola bob").unwrap();
    store.queue(&bob.1.id(), "segundo").unwrap();
    assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap(), 2);
    drop(store);
    assert_eq!(h.join().unwrap(), Ok(2));

    assert_eq!(statuses(&alice.0), ["entregado:hola bob", "entregado:segundo"]);
    assert_eq!(statuses(&bob.0), ["guardado:hola bob", "guardado:segundo"]);
}

#[test]
fn desconocido_es_rechazado() {
    let (bob, mallory) = (device("bob2"), device("mallory"));
    // Mallory conoce la clave de Bob, pero Bob no tiene a Mallory como contacto.
    st(&mallory.0).add_contact("bob", &bob.1.id(), None).unwrap();
    let (addr, h) = listen_once(bob.0.clone());

    let store = st(&mallory.0);
    store.queue(&bob.1.id(), "spam").unwrap();
    assert!(deliver(&store, &mallory.1, &bob.1.id(), &addr).is_err());
    assert!(h.join().unwrap().unwrap_err().contains("clave desconocida"));
    assert!(statuses(&bob.0).is_empty());
}

#[test]
fn suplantacion_detectada_y_mensaje_queda_pendiente() {
    let (alice, bob, mallory) = (device("alice3"), device("bob3"), device("mallory3"));
    befriend(&alice, "alice", &bob, "bob");
    // Mallory se pone en la dirección donde Alice espera a Bob.
    let (addr, h) = listen_once(mallory.0.clone());

    let store = st(&alice.0);
    store.queue(&bob.1.id(), "solo para bob").unwrap();
    let err = deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap_err().to_string();
    assert!(err.contains("suplantación"), "{err}");
    let _ = h.join();
    drop(store);
    assert_eq!(statuses(&alice.0), ["pendiente:solo para bob"]);
    assert!(statuses(&mallory.0).is_empty());
}

#[test]
fn intermediario_solo_ve_datos_cifrados() {
    let (alice, bob) = (device("alice4"), device("bob4"));
    befriend(&alice, "alice", &bob, "bob");
    let (bob_addr, h) = listen_once(bob.0.clone());

    // Un "relé" que reenvía bytes entre ambos y apunta todo lo que ve.
    let relay = TcpListener::bind("127.0.0.1:0").unwrap();
    let relay_addr = relay.local_addr().unwrap().to_string();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    let r = thread::spawn(move || {
        let a = relay.accept().unwrap().0;
        let b = TcpStream::connect(bob_addr).unwrap();
        let pipe = |mut from: TcpStream, mut to: TcpStream, seen: Arc<Mutex<Vec<u8>>>| {
            thread::spawn(move || {
                let mut buf = [0; 4096];
                while let Ok(n @ 1..) = from.read(&mut buf) {
                    seen.lock().unwrap().extend_from_slice(&buf[..n]);
                    if to.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                let _ = to.shutdown(std::net::Shutdown::Write);
            })
        };
        let x = pipe(a.try_clone().unwrap(), b.try_clone().unwrap(), seen2.clone());
        let y = pipe(b, a, seen2);
        x.join().unwrap();
        y.join().unwrap();
    });

    let store = st(&alice.0);
    store.queue(&bob.1.id(), "secreto-123").unwrap();
    assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &relay_addr).unwrap(), 1);
    drop(store);
    assert_eq!(h.join().unwrap(), Ok(1));
    r.join().unwrap();

    let seen = seen.lock().unwrap();
    assert!(seen.len() > 100, "el relé debió ver tráfico");
    assert!(!seen.windows(11).any(|w| w == b"secreto-123"), "el relé vio el texto en claro");
    assert!(!seen.windows(4).any(|w| w == b"\"id\""), "el relé vio la estructura del mensaje");
}

#[test]
fn sin_conexion_queda_pendiente_y_luego_se_entrega_sin_duplicar() {
    let (alice, bob) = (device("alice5"), device("bob5"));
    befriend(&alice, "alice", &bob, "bob");
    let store = st(&alice.0);
    store.queue(&bob.1.id(), "offline").unwrap();

    // Bob apagado: puerto sin nadie escuchando.
    let dead = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string();
    assert!(deliver(&store, &alice.1, &bob.1.id(), &dead).is_err());
    assert_eq!(store.pending(&bob.1.id()).unwrap().len(), 1);

    // Bob vuelve; además ya tenía ese mismo mensaje (reintento tras perder la confirmación).
    let (id, body, _) = store.pending(&bob.1.id()).unwrap().remove(0);
    st(&bob.0).save_incoming(&alice.1.id(), &id, &body).unwrap();
    let (addr, h) = listen_once(bob.0.clone());
    assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap(), 1);
    assert_eq!(h.join().unwrap(), Ok(1));
    drop(store);
    assert_eq!(statuses(&alice.0), ["entregado:offline"]);
    assert_eq!(statuses(&bob.0), ["guardado:offline"]);
}

#[test]
fn identidad_danada_se_detecta() {
    let (dir, _) = device("danada");
    assert!(Identity::create(&dir, &Plain).is_err(), "no debe pisar una identidad existente");
    assert!(Identity::load(&dir, &Plain).is_ok());
    // Lo que dejó el apagón en el TECNO: 64 bytes a cero.
    std::fs::write(dir.join("identity.key"), [0u8; 64]).unwrap();
    assert!(Identity::load(&dir, &Plain).err().unwrap().to_string().contains("dañada"));
}

#[test]
fn base_antigua_se_migra_y_no_se_cambia_la_clave_en_silencio() {
    let (dir, _) = device("migracion");
    // Base de datos como la de los teléfonos de la Fase 4: sin columna de dirección.
    let old = rusqlite::Connection::open(dir.join("mesh.db")).unwrap();
    old.execute_batch("CREATE TABLE contacts(name TEXT PRIMARY KEY, key TEXT NOT NULL UNIQUE);").unwrap();
    old.execute("INSERT INTO contacts VALUES ('ana', ?1)", ["a".repeat(64)]).unwrap();
    drop(old);

    let store = st(&dir);
    store.add_contact("ana", &"a".repeat(64), Some("10.0.0.1:7801")).unwrap();
    assert_eq!(store.contacts().unwrap(), [("ana".into(), "a".repeat(64), Some("10.0.0.1:7801".into()))]);
    drop(store);
    st(&dir); // abrir otra vez no repite la migración

    let store = st(&dir);
    let err = store.add_contact("ana", &"b".repeat(64), None).err().unwrap().to_string();
    assert!(err.contains("otra clave"), "{err}");
}

#[test]
fn invitacion_ida_y_vuelta() {
    let (_, me) = device("invita");
    let inv = mobilemesh_core::invite(&me, "192.168.20.3:7801");
    assert_eq!(mobilemesh_core::parse_invite(&inv).unwrap(), (me.id(), "192.168.20.3:7801".into()));
    assert!(mobilemesh_core::parse_invite("hola").is_err());
    assert!(mobilemesh_core::parse_invite(&inv.replace("7801", "puerto")).is_err());
}

#[test]
fn identidad_restaurada_conserva_su_clave_y_sus_contactos_la_aceptan() {
    let (alice, bob) = (device("alice6"), device("bob6"));
    befriend(&alice, "alice", &bob, "bob");
    let code = mobilemesh_core::recovery::export(&alice.1, "una frase segura").unwrap();

    // Alice pierde el teléfono: carpeta nueva, se restaura desde la copia.
    let nuevo = alice.0.with_extension("nuevo");
    let _ = std::fs::remove_dir_all(&nuevo);
    let restored = mobilemesh_core::recovery::import(&code, "una frase segura").unwrap();
    restored.save(&nuevo, &Plain).unwrap();
    st(&nuevo).add_contact("bob", &bob.1.id(), None).unwrap();
    assert_eq!(Identity::load(&nuevo, &Plain).unwrap().id(), alice.1.id());

    let (addr, h) = listen_once(bob.0.clone());
    let store = st(&nuevo);
    store.queue(&bob.1.id(), "he vuelto").unwrap();
    assert_eq!(deliver(&store, &restored, &bob.1.id(), &addr).unwrap(), 1);
    assert_eq!(h.join().unwrap(), Ok(1));
}

/// Vincula un dispositivo nuevo en `dir` a la cuenta de `primary`, como harían los dos QR.
fn link(primary: &Identity, dir: &std::path::Path) -> Identity {
    let p = mobilemesh_core::Pending::create(dir, &Plain).unwrap();
    let request = mobilemesh_core::link_request(&p);
    let cert = mobilemesh_core::approve_link(primary, &request).unwrap();
    let id = mobilemesh_core::complete_link(p, dir, &Plain, &cert).unwrap();
    st(dir);
    id
}

#[test]
fn dispositivo_vinculado_habla_en_nombre_de_la_cuenta() {
    let (alice, bob) = (device("alice7"), device("bob7"));
    befriend(&alice, "alice", &bob, "bob");
    let tablet = alice.0.with_extension("tablet");
    let _ = std::fs::remove_dir_all(&tablet);
    let t = link(&alice.1, &tablet);
    assert_eq!(t.id(), alice.1.id(), "misma cuenta");
    assert_ne!(t.public, alice.1.public, "otro dispositivo");
    assert_eq!(Identity::load(&tablet, &Plain).unwrap().id(), alice.1.id(), "se guardó bien");

    // La tableta escribe a Bob: Bob la acepta como Alice sin haber hecho nada.
    st(&tablet).add_contact("bob", &bob.1.id(), None).unwrap();
    let (addr, h) = listen_once(bob.0.clone());
    let store = st(&tablet);
    store.queue(&bob.1.id(), "desde la tableta").unwrap();
    assert_eq!(deliver(&store, &t, &bob.1.id(), &addr).unwrap(), 1);
    assert_eq!(h.join().unwrap(), Ok(1));
    assert_eq!(statuses(&bob.0), ["guardado:desde la tableta"]);

    // Y Bob escribe a la tableta esperando la cuenta de Alice.
    let (addr, h) = listen_once(tablet.clone());
    let store = st(&bob.0);
    store.queue(&alice.1.id(), "hola tableta").unwrap();
    assert_eq!(deliver(&store, &bob.1, &alice.1.id(), &addr).unwrap(), 1);
    assert_eq!(h.join().unwrap(), Ok(1));
}

#[test]
fn certificado_falso_o_ajeno_se_rechaza() {
    let (alice, bob, mallory) = (device("alice8"), device("bob8"), device("mallory8"));
    befriend(&alice, "alice", &bob, "bob");

    // Mallory dice ser Alice con un certificado firmado por su propia cuenta.
    let mut fake = mallory.1.clone();
    fake.account = alice.1.account;
    st(&mallory.0).add_contact("bob", &bob.1.id(), None).unwrap();
    let (addr, h) = listen_once(bob.0.clone());
    let store = st(&mallory.0);
    store.queue(&bob.1.id(), "soy alice").unwrap();
    assert!(deliver(&store, &fake, &bob.1.id(), &addr).is_err());
    assert!(h.join().unwrap().unwrap_err().contains("certificado"));
    assert!(statuses(&bob.0).is_empty());

    // Un certificado emitido para otro dispositivo no se puede instalar.
    let d1 = alice.0.with_extension("d1");
    let d2 = alice.0.with_extension("d2");
    for d in [&d1, &d2] {
        let _ = std::fs::remove_dir_all(d);
    }
    let p1 = mobilemesh_core::Pending::create(&d1, &Plain).unwrap();
    let p2 = mobilemesh_core::Pending::create(&d2, &Plain).unwrap();
    let cert_for_1 = mobilemesh_core::approve_link(&alice.1, &mobilemesh_core::link_request(&p1)).unwrap();
    assert!(mobilemesh_core::complete_link(p2, &d2, &Plain, &cert_for_1).is_err());

    // Un dispositivo vinculado no puede vincular otros ni crear la copia (no tiene la clave de la cuenta).
    let linked = mobilemesh_core::complete_link(p1, &d1, &Plain, &cert_for_1).unwrap();
    let d3 = alice.0.with_extension("d3");
    let _ = std::fs::remove_dir_all(&d3);
    let p3 = mobilemesh_core::Pending::create(&d3, &Plain).unwrap();
    assert!(mobilemesh_core::approve_link(&linked, &mobilemesh_core::link_request(&p3)).is_err());
    assert!(mobilemesh_core::recovery::export(&linked, "contraseña larga").is_err());
}

#[test]
fn identidad_antigua_se_convierte_en_cuenta() {
    let (dir, _) = device("antigua");
    // Archivo de la Fase 4: solo la clave X25519 del dispositivo (64 bytes).
    let kp = snow::Builder::new(mobilemesh_core::identity::NOISE.parse().unwrap()).generate_keypair().unwrap();
    std::fs::write(dir.join("identity.key"), [kp.private.as_slice(), &kp.public].concat()).unwrap();
    let id = Identity::load(&dir, &Plain).unwrap();
    assert_eq!(id.public, kp.public, "conserva la clave del dispositivo");
    assert!(id.account_secret.is_some(), "pasa a ser el principal de una cuenta nueva");
    assert_eq!(Identity::load(&dir, &Plain).unwrap().id(), id.id(), "y queda guardada así");
}

// ---------- Fase 5: cifrado de extremo a extremo ----------

use mobilemesh_core::e2e;

/// Dos dispositivos con sesión abierta directamente con las funciones del ratchet (sin red).
fn ratchet_pair(tag: &str) -> ((PathBuf, Identity), (PathBuf, Identity)) {
    let (a, b) = (device(&format!("ra-{tag}")), device(&format!("rb-{tag}")));
    let (identity, one_time) = e2e::publish_keys(&st(&b.0)).unwrap();
    e2e::start_session(&st(&a.0), "dev-b", &identity, &one_time).unwrap();
    (a, b)
}

#[test]
fn cada_mensaje_usa_una_clave_distinta_y_el_ratchet_avanza() {
    let (a, b) = ratchet_pair("avanza");
    let (k1, c1) = e2e::encrypt(&st(&a.0), "dev-b", "m1", "hola", None).unwrap();
    let (_, c2) = e2e::encrypt(&st(&a.0), "dev-b", "m1", "hola", None).unwrap();
    assert_ne!(c1, c2, "el mismo texto cifrado dos veces no se parece");
    assert_eq!(k1, 0, "el primer mensaje lleva lo necesario para abrir la sesión");
    assert_eq!(e2e::decrypt(&st(&b.0), "dev-a", "m1", k1, &c1).unwrap().map(|p| p.0).as_deref(), Some("hola"));

    // Bob responde y Alice, al recibirlo, pasa a mensajes normales del ratchet.
    let (k3, c3) = e2e::encrypt(&st(&b.0), "dev-a", "m3", "respuesta", None).unwrap();
    assert_eq!(k3, 1);
    assert_eq!(e2e::decrypt(&st(&a.0), "dev-b", "m3", k3, &c3).unwrap().map(|p| p.0).as_deref(), Some("respuesta"));
    let (k4, _) = e2e::encrypt(&st(&a.0), "dev-b", "m4", "otra", None).unwrap();
    assert_eq!(k4, 1);
}

#[test]
fn mensaje_alterado_repetido_o_con_otro_id_se_rechaza() {
    let (a, b) = ratchet_pair("altera");
    let (k, c) = e2e::encrypt(&st(&a.0), "dev-b", "m1", "intacto", None).unwrap();
    let mut bad = c.clone().into_bytes();
    let i = bad.len() / 2;
    bad[i] = if bad[i] == b'A' { b'B' } else { b'A' };
    let tampered = e2e::decrypt(&st(&b.0), "dev-a", "m1", k, &String::from_utf8(bad).unwrap());
    assert!(!matches!(tampered, Ok(Some(_))), "alterado: no se acepta");
    assert!(e2e::decrypt(&st(&b.0), "dev-a", "otro-id", k, &c).is_err(), "identificador cambiado por fuera");
    assert_eq!(e2e::decrypt(&st(&b.0), "dev-a", "m1", k, &c).unwrap().map(|p| p.0).as_deref(), Some("intacto"));
    // Repetido: no se acepta; se pide abrir una sesión nueva en vez de dejar el canal roto.
    assert!(!matches!(e2e::decrypt(&st(&b.0), "dev-a", "m1", k, &c), Ok(Some(_))), "repetido");
}

#[test]
fn cambio_de_clave_de_cifrado_de_un_dispositivo_se_detecta() {
    let (a, _) = ratchet_pair("pin");
    let other = device("rc-pin");
    let (identity, one_time) = e2e::publish_keys(&st(&other.0)).unwrap();
    let err = e2e::start_session(&st(&a.0), "dev-b", &identity, &one_time).err().unwrap().to_string();
    assert!(err.contains("cambió"), "{err}");
}

#[test]
fn la_base_de_datos_copiada_no_revela_el_texto() {
    let (alice, bob) = (device("alice9"), device("bob9"));
    befriend(&alice, "alice", &bob, "bob");
    let (addr, h) = listen_once(bob.0.clone());
    let store = st(&alice.0);
    store.queue(&bob.1.id(), "texto-secreto-xyz").unwrap();
    assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap(), 1);
    drop(store);
    assert_eq!(h.join().unwrap(), Ok(1));
    for d in [&alice.0, &bob.0] {
        let raw = std::fs::read(d.join("mesh.db")).unwrap();
        assert!(!raw.windows(17).any(|w| w == b"texto-secreto-xyz"), "texto en claro en {}", d.display());
        assert_eq!(statuses(d).len(), 1, "pero con la clave sí se lee");
    }
    // Abierta con otra clave (otro dispositivo), no se lee.
    let ajena = Store::open(&bob.0, [9u8; 32]).unwrap();
    assert!(ajena.list().unwrap()[0].body.contains("no se puede descifrar"));
}

#[test]
fn sesion_perdida_se_rehace_sola() {
    let (alice, bob) = (device("alice10"), device("bob10"));
    befriend(&alice, "alice", &bob, "bob");
    let send = |text: &str| {
        let (addr, h) = listen_once(bob.0.clone());
        let store = st(&alice.0);
        if !text.is_empty() {
            store.queue(&bob.1.id(), text).unwrap();
        }
        let r = deliver(&store, &alice.1, &bob.1.id(), &addr).map_err(|e| e.to_string());
        let _ = h.join();
        r
    };
    assert_eq!(send("uno"), Ok(1));
    // Bob pierde el estado del cifrado (p. ej. base de datos dañada) pero conserva su identidad.
    let db = rusqlite::Connection::open(bob.0.join("mesh.db")).unwrap();
    db.execute_batch("DELETE FROM olm_sessions;").unwrap();
    drop(db);
    assert!(send("dos").unwrap_err().contains("perdió la sesión"));
    assert_eq!(send(""), Ok(1), "el reintento abre una sesión nueva y entrega");
    assert_eq!(statuses(&bob.0), ["guardado:uno", "guardado:dos"]);
}

// ---------- Fase 6: archivos ----------

/// Un relé que corta la conexión después de dejar pasar `limit` bytes hacia el destino.
/// Devuelve su dirección y cuántos bytes dejó pasar hacia el destino.
fn cutting_relay(dest: String, limit: usize) -> (String, thread::JoinHandle<usize>) {
    let relay = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = relay.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let (mut a, _) = relay.accept().unwrap();
        let mut b = TcpStream::connect(dest).unwrap();
        let (mut a2, mut b2) = (a.try_clone().unwrap(), b.try_clone().unwrap());
        let back = thread::spawn(move || std::io::copy(&mut b2, &mut a2).ok());
        let mut buf = [0u8; 4096];
        let mut sent = 0;
        while let Ok(n @ 1..) = a.read(&mut buf) {
            let n = n.min(limit - sent);
            if b.write_all(&buf[..n]).is_err() {
                break;
            }
            sent += n;
            if sent >= limit {
                break;
            }
        }
        let _ = a.shutdown(std::net::Shutdown::Both);
        let _ = b.shutdown(std::net::Shutdown::Both);
        let _ = back.join();
        sent
    });
    (addr, h)
}

fn file_pair(tag: &str) -> ((PathBuf, Identity), (PathBuf, Identity)) {
    let (a, b) = (device(&format!("fa-{tag}")), device(&format!("fb-{tag}")));
    befriend(&a, "alice", &b, "bob");
    (a, b)
}

fn sample(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 + i / 300) as u8).collect()
}

/// Exporta el archivo recibido por Bob (el único) y lo devuelve.
fn received(bob: &(PathBuf, Identity), alice: &Identity) -> Vec<u8> {
    let store = st(&bob.0);
    let row = store.list().unwrap().into_iter().find(|r| r.file.is_some()).expect("Bob tiene un archivo");
    let out = bob.0.join("exportado");
    mobilemesh_core::export_file(&store, &alice.id(), false, &row.id, &out).unwrap();
    std::fs::read(out).unwrap()
}

#[test]
fn archivos_de_varios_tamanos_llegan_integros() {
    for len in [0, 1, 16 * 1024, 300_000, 2_000_000] {
        let (alice, bob) = file_pair(&format!("t{len}"));
        let src = alice.0.join("original.bin");
        std::fs::write(&src, sample(len)).unwrap();
        let store = st(&alice.0);
        mobilemesh_core::queue_file(&store, &bob.1.id(), &src, "foto.jpg", "image/jpeg").unwrap();
        let (addr, h) = listen_once(bob.0.clone());
        assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap(), 1, "tamaño {len}");
        assert_eq!(h.join().unwrap(), Ok(1));
        assert_eq!(received(&bob, &alice.1), sample(len), "tamaño {len}");
        assert!(src.exists(), "el original del remitente no se toca");
        let row = &st(&bob.0).list().unwrap()[0];
        assert_eq!((row.status.as_str(), row.progress, row.body.as_str()), ("guardado", 100, "foto.jpg"));
        assert_eq!(st(&alice.0).list().unwrap()[0].status, "entregado");
    }
}

#[test]
fn transferencia_cortada_continua_donde_se_quedo() {
    let (alice, bob) = file_pair("corte");
    let src = alice.0.join("video.bin");
    std::fs::write(&src, sample(1_500_000)).unwrap();
    let store = st(&alice.0);
    mobilemesh_core::queue_file(&store, &bob.1.id(), &src, "video.mp4", "video/mp4").unwrap();

    // Primer intento: la conexión se corta cuando ha pasado algo menos de la mitad.
    let (bob_addr, h) = listen_once(bob.0.clone());
    let (relay, cut) = cutting_relay(bob_addr, 700_000);
    assert!(deliver(&store, &alice.1, &bob.1.id(), &relay).is_err());
    cut.join().unwrap();
    let _ = h.join();
    let row = &st(&bob.0).list().unwrap()[0];
    assert_eq!(row.status, "recibiendo", "incompleto: no se da por recibido");
    assert!(row.progress > 0 && row.progress < 100);
    assert_eq!(st(&alice.0).list().unwrap()[0].status, "pendiente");

    // Segundo intento: solo viaja lo que falta.
    let (bob_addr, h) = listen_once(bob.0.clone());
    let (relay, passed) = cutting_relay(bob_addr, usize::MAX);
    assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &relay).unwrap(), 1);
    assert_eq!(h.join().unwrap(), Ok(1));
    let passed = passed.join().unwrap();
    // El archivo entero ocupa unos 2 MB por la red (base64); el corte dejó pasar 700 KB.
    assert!(passed < 1_500_000, "reenvió casi todo: {passed} bytes");
    assert_eq!(received(&bob, &alice.1), sample(1_500_000));
}

#[test]
fn archivo_alterado_se_rechaza() {
    let (alice, bob) = file_pair("altera");
    let src = alice.0.join("doc.pdf");
    std::fs::write(&src, sample(100_000)).unwrap();
    let store = st(&alice.0);
    let id = mobilemesh_core::queue_file(&store, &bob.1.id(), &src, "doc.pdf", "application/pdf").unwrap();
    // Se estropea un byte del archivo cifrado que va a salir.
    let blob = store.blob(&bob.1.id(), true, &id).unwrap();
    let mut b = std::fs::read(&blob).unwrap();
    b[50_000] ^= 1;
    std::fs::write(&blob, b).unwrap();

    let (addr, h) = listen_once(bob.0.clone());
    let err = deliver(&store, &alice.1, &bob.1.id(), &addr).err().unwrap().to_string();
    let _ = h.join();
    assert!(err.contains("rechazó el archivo"), "{err}");
    assert_eq!(st(&alice.0).list().unwrap()[0].status, "error");
    let row = &st(&bob.0).list().unwrap()[0];
    assert_eq!(row.status, "recibiendo", "no se da por recibido");
    assert!(mobilemesh_core::export_file(&st(&bob.0), &alice.1.id(), false, &row.id, &bob.0.join("x")).is_err());
}

#[test]
fn archivo_ya_recibido_no_se_reenvia() {
    let (alice, bob) = file_pair("dup");
    let src = alice.0.join("foto.jpg");
    std::fs::write(&src, sample(500_000)).unwrap();
    let store = st(&alice.0);
    let id = mobilemesh_core::queue_file(&store, &bob.1.id(), &src, "foto.jpg", "image/jpeg").unwrap();
    let (addr, h) = listen_once(bob.0.clone());
    deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap();
    h.join().unwrap().unwrap();
    // Alice no recibió la confirmación (simulado) y lo intenta otra vez.
    store.set_status(&bob.1.id(), true, &id, "pendiente").unwrap();
    let (bob_addr, h) = listen_once(bob.0.clone());
    let (relay, passed) = cutting_relay(bob_addr, usize::MAX);
    assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &relay).unwrap(), 1);
    h.join().unwrap().unwrap();
    assert!(passed.join().unwrap() < 20_000, "no debe volver a mandar los trozos");
    assert_eq!(st(&bob.0).list().unwrap().len(), 1, "sin duplicados");
}

// ---------- Fase 7: sincronización entre dispositivos de la cuenta ----------

use mobilemesh_core::sync::{self, Policy};

#[test]
fn dispositivos_de_la_cuenta_sincronizan_lo_hecho_sin_conexion() {
    let alice = device("alice11");
    let tablet = alice.0.with_extension("tablet");
    let _ = std::fs::remove_dir_all(&tablet);
    let t = link(&alice.1, &tablet);
    let (da, dt) = (to_hex_dev(&alice.1), to_hex_dev(&t));

    // Sin conexión: cada uno cambia su parte y los dos cambian el saldo partiendo de lo mismo.
    sync::put(&st(&alice.0), &da, Policy::Lww, "perfil", "yo", "nombre", Some("Alice")).unwrap();
    sync::put(&st(&tablet), &dt, Policy::Lww, "perfil", "yo", "estado", Some("en clase")).unwrap();
    for i in 0..400 {
        // Muchas notas para obligar a mandar varias tandas.
        sync::put(&st(&tablet), &dt, Policy::Lww, "notas", &format!("n{i:03}"), "texto", Some(&"x".repeat(100))).unwrap();
    }
    sync::put(&st(&alice.0), &da, Policy::Strict, "caja", "saldo", "valor", Some("80")).unwrap();
    sync::put(&st(&tablet), &dt, Policy::Strict, "caja", "saldo", "valor", Some("150")).unwrap();
    assert!(sync::unsynced(&st(&alice.0), &da).unwrap() > 0);

    let (addr, h) = listen_once(tablet.clone());
    let (got, sent) = mobilemesh_core::sync_with(&st(&alice.0), &alice.1, &addr).unwrap();
    h.join().unwrap().unwrap();
    assert_eq!((got, sent), (402, 2));
    for col in ["perfil", "notas", "caja"] {
        assert_eq!(sync::records(&st(&alice.0), col).unwrap(), sync::records(&st(&tablet), col).unwrap(), "{col}");
    }
    let perfil = sync::records(&st(&alice.0), "perfil").unwrap();
    assert_eq!(perfil[0].fields.len(), 2, "nombre y estado: no se pisan");
    assert_eq!(sync::records(&st(&alice.0), "caja").unwrap()[0].conflicts.len(), 1, "saldo en conflicto");
    assert_eq!(sync::unsynced(&st(&alice.0), &da).unwrap(), 0, "todo confirmado");

    // Una segunda sincronización no mueve nada.
    let (addr, h) = listen_once(tablet.clone());
    assert_eq!(mobilemesh_core::sync_with(&st(&alice.0), &alice.1, &addr).unwrap(), (0, 0));
    h.join().unwrap().unwrap();
}

#[test]
fn un_contacto_no_puede_sincronizar_con_mi_cuenta() {
    let (alice, bob) = (device("alice12"), device("bob12"));
    befriend(&alice, "alice", &bob, "bob");
    sync::put(&st(&alice.0), &to_hex_dev(&alice.1), Policy::Lww, "perfil", "yo", "nombre", Some("privado")).unwrap();
    let (addr, h) = listen_once(alice.0.clone());
    assert!(mobilemesh_core::sync_with(&st(&bob.0), &bob.1, &addr).is_err());
    let _ = h.join();
    assert!(sync::records(&st(&bob.0), "perfil").unwrap().is_empty(), "Bob no recibió nada");
}

fn to_hex_dev(id: &Identity) -> String {
    id.public.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- Fase 8: copias en otros dispositivos ----------

use mobilemesh_core::replica;

/// Alice con algo de historia: Bob de contacto, un mensaje entregado y su perfil.
fn alice_with_data(tag: &str) -> ((PathBuf, Identity), (PathBuf, Identity)) {
    let (alice, bob) = (device(&format!("ra8-{tag}")), device(&format!("rb8-{tag}")));
    befriend(&alice, "alice", &bob, "bob");
    sync::put(&st(&alice.0), &to_hex_dev(&alice.1), Policy::Lww, "perfil", "yo", "nombre", Some("perfil-secreto-alice")).unwrap();
    let store = st(&alice.0);
    store.queue(&bob.1.id(), "mensaje-secreto-para-bob").unwrap();
    let (addr, h) = listen_once(bob.0.clone());
    deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap();
    h.join().unwrap().unwrap();
    (alice, bob)
}

/// Bob atiende una conexión y Alice hace `op` con él como guardián de su copia.
fn with_holder<T>(alice: &(PathBuf, Identity), bob: &(PathBuf, Identity), me: &Identity, dir: &std::path::Path, op: impl FnOnce(&Store, &mobilemesh_core::store::Holder) -> T) -> T {
    let (addr, h) = listen_once(bob.0.clone());
    let store = Store::open(dir, me.db_key()).unwrap();
    store.add_holder("bob", &bob.1.id(), &addr).unwrap();
    let holder = store.holders().unwrap().into_iter().find(|x| x.name == "bob").unwrap();
    let r = op(&store, &holder);
    let _ = h.join();
    let _ = alice;
    r
}

#[test]
fn copia_en_un_amigo_y_recuperacion_tras_perder_el_telefono() {
    let (alice, bob) = alice_with_data("pierde");
    st(&bob.0).set_consent(&alice.1.id(), true).unwrap();
    replica::snapshot(&st(&alice.0), &alice.1).unwrap();
    with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::push(s, &alice.1, h)).unwrap();
    assert!(st(&bob.0).held(&alice.1.id()).unwrap().is_some(), "Bob la guarda");
    // Bob no puede leer nada de lo que guarda.
    for f in std::fs::read_dir(bob.0.join("copias")).unwrap() {
        let raw = std::fs::read(f.unwrap().path()).unwrap();
        assert!(!raw.windows(20).any(|w| w == b"perfil-secreto-alice" || w == b"mensaje-secreto-para"));
    }
    let status = with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::check(s, &alice.1, h)).unwrap();
    assert_eq!(status, "verificada");

    // Alice pierde el teléfono: recupera la cuenta con su código en un teléfono nuevo, vacío.
    let code = mobilemesh_core::recovery::export(&alice.1, "contraseña larga").unwrap();
    let nuevo = alice.0.with_extension("nuevo");
    let _ = std::fs::remove_dir_all(&nuevo);
    let me = mobilemesh_core::recovery::import(&code, "contraseña larga").unwrap();
    me.save(&nuevo, &Plain).unwrap();
    assert!(st(&nuevo).list().unwrap().is_empty());
    let got = with_holder(&alice, &bob, &me, &nuevo, |s, h| replica::restore(s, &me, h)).unwrap();
    assert_eq!((got.contacts, got.messages), (1, 1));
    assert_eq!(statuses(&nuevo), ["entregado:mensaje-secreto-para-bob"]);
    assert_eq!(sync::records(&st(&nuevo), "perfil").unwrap(), sync::records(&st(&alice.0), "perfil").unwrap());
    assert_eq!(st(&nuevo).contact_key("bob").unwrap(), Some(bob.1.id()));
}

#[test]
fn sin_permiso_no_se_guarda_y_un_ajeno_no_descarga() {
    let (alice, bob) = alice_with_data("permiso");
    replica::snapshot(&st(&alice.0), &alice.1).unwrap();
    let err = with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::push(s, &alice.1, h)).err().unwrap().to_string();
    assert!(err.contains("no he aceptado"), "{err}");
    assert!(st(&bob.0).held(&alice.1.id()).unwrap().is_none());

    // Con permiso sí; pero Mallory (otro contacto de Bob) no puede llevarse la copia de Alice.
    st(&bob.0).set_consent(&alice.1.id(), true).unwrap();
    with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::push(s, &alice.1, h)).unwrap();
    let mallory = device("mallory8");
    Store::open(&bob.0, bob.1.db_key()).unwrap().add_contact("mallory", &mallory.1.id(), None).unwrap();
    st(&mallory.0).add_contact("bob", &bob.1.id(), None).unwrap();
    let mut thief = mallory.1.clone();
    thief.account_secret = Some([1u8; 32]); // para poder intentar descifrar algo
    let err = with_holder(&mallory, &bob, &thief, &mallory.0, |s, h| replica::restore(s, &thief, h)).err().unwrap().to_string();
    assert!(err.contains("ninguna copia de tu cuenta"), "{err}");
}

#[test]
fn copia_alterada_o_vieja_se_detecta() {
    let (alice, bob) = alice_with_data("altera");
    st(&bob.0).set_consent(&alice.1.id(), true).unwrap();
    replica::snapshot(&st(&alice.0), &alice.1).unwrap();
    with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::push(s, &alice.1, h)).unwrap();

    // Bob (o su disco) estropea un byte: se nota sin descargar nada, y al recuperar se rechaza.
    let held = st(&bob.0).replica_path(Some(&alice.1.id())).unwrap();
    let mut b = std::fs::read(&held).unwrap();
    b[100] ^= 1;
    std::fs::write(&held, &b).unwrap();
    let status = with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::check(s, &alice.1, h)).unwrap();
    assert_eq!(status, "dañada");
    assert!(with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::restore(s, &alice.1, h)).is_err());
    b[100] ^= 1;
    std::fs::write(&held, &b).unwrap();

    // Bob intenta hacer pasar la copia por otra versión: no cuela.
    let db = rusqlite::Connection::open(bob.0.join("mesh.db")).unwrap();
    db.execute("UPDATE replicas_held SET version = version + 1", []).unwrap();
    drop(db);
    let err = with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::restore(s, &alice.1, h)).err().unwrap().to_string();
    assert!(err.contains("alterada"), "{err}");
}

#[test]
fn borrar_la_copia_se_reintenta_si_no_contesta() {
    let (alice, bob) = alice_with_data("borra");
    st(&bob.0).set_consent(&alice.1.id(), true).unwrap();
    replica::snapshot(&st(&alice.0), &alice.1).unwrap();
    with_holder(&alice, &bob, &alice.1, &alice.0, |s, h| replica::push(s, &alice.1, h)).unwrap();

    // Bob está apagado: queda «borrando».
    let store = st(&alice.0);
    let dead = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string();
    store.add_holder("bob", &bob.1.id(), &dead).unwrap();
    let h = store.holders().unwrap().remove(0);
    assert!(replica::delete(&store, &alice.1, &h).is_err());
    assert_eq!(store.holders().unwrap()[0].status, "borrando");
    assert!(st(&bob.0).held(&alice.1.id()).unwrap().is_some());

    // Bob vuelve: el borrado se completa y Alice deja de tenerlo como guardián.
    let (addr, j) = listen_once(bob.0.clone());
    let mut h = store.holders().unwrap().remove(0);
    h.addr = addr;
    replica::delete(&store, &alice.1, &h).unwrap();
    j.join().unwrap().unwrap();
    assert!(st(&bob.0).held(&alice.1.id()).unwrap().is_none());
    assert!(store.holders().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(bob.0.join("copias")).unwrap().count(), 0, "sin restos en disco");
}

// ---------- Fase 9: relé ----------

use mobilemesh_core::relay::{self as rele, Relay};

fn start_relay() -> (String, Relay) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let relay = Relay::default();
    let r = relay.clone();
    thread::spawn(move || r.serve(l));
    (addr, relay)
}

/// El dispositivo `dev` se registra en el relé con su buzón y atiende las llamadas que lleguen.
fn register(dev: &(PathBuf, Identity), relay: &str, secret: &str) -> String {
    let (dir, me) = (dev.0.clone(), dev.1.clone());
    let (relay, secret2) = (relay.to_string(), secret.to_string());
    thread::spawn(move || {
        rele::listen(relay, secret2, move |stream| {
            let _ = serve(&Store::open(&dir, me.db_key()).unwrap(), &me, stream);
        })
    });
    let id = rele::mailbox_id(secret);
    thread::sleep(std::time::Duration::from_millis(300));
    id
}

#[test]
fn el_rele_une_dispositivos_sin_conexion_directa() {
    let (alice, bob) = (device("alice13"), device("bob13"));
    befriend(&alice, "alice", &bob, "bob");
    let (relay, r) = start_relay();
    let id = register(&bob, &relay, &"s".repeat(64));
    // La ruta directa no funciona (como entre redes distintas): se usa el relé.
    let route = format!("127.0.0.1:9|relay={relay}/{id}");
    let store = st(&alice.0);
    store.queue(&bob.1.id(), "por el relé").unwrap();
    assert_eq!(deliver(&store, &alice.1, &bob.1.id(), &route).unwrap(), 1);
    thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(statuses(&bob.0), ["guardado:por el relé"]);
    assert!(r.forwarded.load(std::sync::atomic::Ordering::Relaxed) > 0, "pasó por el relé");

    // Un buzón que nadie registró no lleva a ninguna parte.
    let err = deliver(&store, &alice.1, &bob.1.id(), &format!("relay={relay}/{}", "0".repeat(32)));
    assert!(err.is_ok(), "sin pendientes no conecta");
    store.queue(&bob.1.id(), "otro").unwrap();
    let err = deliver(&store, &alice.1, &bob.1.id(), &format!("relay={relay}/{}", "0".repeat(32))).err().unwrap().to_string();
    assert!(err.contains("no está conectado"), "{err}");
    // El buzón no revela su secreto y la invitación lo acepta como ruta válida.
    assert_ne!(id, "s".repeat(64));
    let inv = mobilemesh_core::invite(&bob.1, &route);
    assert_eq!(mobilemesh_core::parse_invite(&inv).unwrap().1, route);
}

#[test]
fn mensajes_cortos_ocupan_lo_mismo_en_la_red() {
    let sizes: Vec<usize> = ["si", "no puedo, mañana"]
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let (alice, bob) = (device(&format!("pad-a{i}")), device(&format!("pad-b{i}")));
            befriend(&alice, "alice", &bob, "bob");
            let store = st(&alice.0);
            store.queue(&bob.1.id(), text).unwrap();
            let (bob_addr, h) = listen_once(bob.0.clone());
            let (relay, passed) = cutting_relay(bob_addr, usize::MAX);
            deliver(&store, &alice.1, &bob.1.id(), &relay).unwrap();
            h.join().unwrap().unwrap();
            passed.join().unwrap()
        })
        .collect();
    assert_eq!(sizes[0], sizes[1], "el tamaño no delata el mensaje");
}

#[test]
fn un_dispositivo_propio_que_cambia_de_red_no_se_duplica() {
    let (dir, _) = device("cambia-red");
    let store = st(&dir);
    store.add_my_device("nubia", "192.168.20.3:7801").unwrap();
    store.add_my_device("nubia", "100.98.239.102:7801|relay=bore.pub:5515/919bf22c641d3fcb06111f9b87b37aab").unwrap();
    assert_eq!(store.my_devices().unwrap(), [("nubia".to_string(), "100.98.239.102:7801|relay=bore.pub:5515/919bf22c641d3fcb06111f9b87b37aab".to_string())]);
}

// ---------- Fase 10: permisos ----------

use mobilemesh_core::access;

fn send_one(from: &(PathBuf, Identity), to: &(PathBuf, Identity), me: &Identity, text: &str) -> Result<usize, String> {
    let store = Store::open(&from.0, me.db_key()).unwrap();
    store.queue(&to.1.id(), text).unwrap();
    let (addr, h) = listen_once(to.0.clone());
    let r = deliver(&store, me, &to.1.id(), &addr).map_err(|e| e.to_string());
    let _ = h.join();
    r
}

#[test]
fn dispositivo_retirado_es_rechazado_por_todos() {
    let (alice, bob) = (device("alice14"), device("bob14"));
    befriend(&alice, "alice", &bob, "bob");
    let tablet_dir = alice.0.with_extension("tablet");
    let _ = std::fs::remove_dir_all(&tablet_dir);
    let tablet = link(&alice.1, &tablet_dir);
    st(&tablet_dir).add_contact("bob", &bob.1.id(), None).unwrap();
    let tab = (tablet_dir.clone(), tablet.clone());
    assert_eq!(send_one(&tab, &bob, &tablet, "desde la tableta"), Ok(1));

    // La tableta se pierde: Alice la retira y, al hablar con Bob, Bob se entera.
    let list = access::revoke(&st(&alice.0), &alice.1, &to_hex_dev(&tablet)).unwrap();
    assert_eq!(list.version, 1);
    assert_eq!(send_one(&alice, &bob, &alice.1, "he retirado mi tableta"), Ok(1));
    // Bob corta la conexión en cuanto ve que es un dispositivo retirado.
    assert!(send_one(&tab, &bob, &tablet, "soy el ladrón").is_err());
    assert!(!statuses(&bob.0).iter().any(|s| s.contains("ladrón")), "Bob no guardó nada del retirado");

    // Tampoco puede sincronizar con los dispositivos de la cuenta.
    let (addr, h) = listen_once(alice.0.clone());
    assert!(mobilemesh_core::sync_with(&st(&tablet_dir), &tablet, &addr).is_err());
    let _ = h.join();

    // Una lista falsa (firmada por otra cuenta) no se acepta, y un vinculado no puede retirar.
    let fake = access::sign(&bob.1, &access::Revocations { account: alice.1.id(), version: 9, revoked: vec![] }).unwrap();
    assert!(access::verify(&fake, &alice.1.id()).is_err());
    assert!(access::revoke(&st(&tablet_dir), &tablet, &to_hex_dev(&alice.1)).is_err());
}

#[test]
fn contacto_bloqueado_no_puede_enviar() {
    let (alice, bob) = (device("alice15"), device("bob15"));
    befriend(&alice, "alice", &bob, "bob");
    st(&bob.0).set_blocked(&alice.1.id(), true).unwrap();
    assert!(send_one(&alice, &bob, &alice.1, "¿me lees?").is_err());
    assert!(statuses(&bob.0).is_empty());
    // Y Bob tampoco le escribe por error.
    let store = st(&bob.0);
    store.queue(&alice.1.id(), "x").unwrap();
    assert!(deliver(&store, &bob.1, &alice.1.id(), "127.0.0.1:9").unwrap_err().to_string().contains("bloqueado"));
}

#[test]
fn demasiados_mensajes_seguidos_se_frenan_sin_perderse() {
    let (alice, bob) = (device("alice16"), device("bob16"));
    befriend(&alice, "alice", &bob, "bob");
    let store = st(&alice.0);
    for i in 0..access::MESSAGES_PER_MINUTE + 5 {
        store.queue(&bob.1.id(), &format!("m{i}")).unwrap();
    }
    let (addr, h) = listen_once(bob.0.clone());
    let err = deliver(&store, &alice.1, &bob.1.id(), &addr).unwrap_err().to_string();
    let _ = h.join();
    assert!(err.starts_with(access::LIMIT), "{err}");
    assert_eq!(statuses(&bob.0).len(), access::MESSAGES_PER_MINUTE);
    assert_eq!(store.pending(&bob.1.id()).unwrap().len(), 5, "los que pasan del límite esperan, no se pierden");
}

#[test]
fn lista_compartida_respeta_los_permisos() {
    let (alice, bob, carol) = (device("alice17"), device("bob17"), device("carol17"));
    befriend(&alice, "alice", &bob, "bob");
    befriend(&alice, "alice", &carol, "carol");
    let lista = "lista-alice".to_string();
    let (a, b) = (alice.1.id(), bob.1.id());
    let (da, db) = (to_hex_dev(&alice.1), to_hex_dev(&bob.1));
    // Alice comparte su lista con Bob, solo para leer; Bob acepta lo que escriba Alice.
    st(&alice.0).set_share(&lista, &b, Some(false)).unwrap();
    st(&bob.0).set_share(&lista, &a, Some(true)).unwrap();
    sync::put_as(&st(&alice.0), &da, &a, Policy::Lww, &lista, "i1", "texto", Some("pan")).unwrap();
    sync::put_as(&st(&alice.0), &da, &a, Policy::Lww, "perfil", "yo", "nombre", Some("privado")).unwrap();

    let sync_bob = || {
        let (addr, h) = listen_once(alice.0.clone());
        let r = mobilemesh_core::sync_shared(&st(&bob.0), &bob.1, &addr, &a).unwrap();
        h.join().unwrap().unwrap();
        r
    };
    assert_eq!(sync_bob().0, 1, "Bob recibe la lista");
    assert_eq!(sync::records(&st(&bob.0), &lista).unwrap()[0].fields[0].1, "pan");
    assert!(sync::records(&st(&bob.0), "perfil").unwrap().is_empty(), "lo no compartido nunca sale");

    // Bob (solo lectura) intenta añadir algo, y además firmar un cambio como si fuera de Alice.
    sync::put_as(&st(&bob.0), &db, &b, Policy::Lww, &lista, "i2", "texto", Some("cerveza")).unwrap();
    sync::put_as(&st(&bob.0), &db, &a, Policy::Lww, &lista, "i1", "texto", Some("nada")).unwrap();
    sync_bob();
    let rows = sync::records(&st(&alice.0), &lista).unwrap();
    assert_eq!(rows.len(), 1, "Alice rechaza lo de Bob: no tiene permiso de escritura");
    assert_eq!(rows[0].fields[0].1, "pan", "ni un cambio a nombre de otro");

    // Alice le da permiso de escritura: ahora sí entra lo de Bob, pero lo falsificado sigue fuera.
    st(&alice.0).set_share(&lista, &b, Some(true)).unwrap();
    sync::put_as(&st(&bob.0), &db, &b, Policy::Lww, &lista, "i3", "texto", Some("leche")).unwrap();
    sync_bob();
    let mut items: Vec<String> = sync::records(&st(&alice.0), &lista).unwrap().into_iter().map(|r| r.fields[0].1.clone()).collect();
    items.sort();
    assert_eq!(items, ["cerveza", "leche", "pan"]);

    // Carol pide la lista sin que Alice se la haya compartido: no recibe nada.
    st(&carol.0).set_share(&lista, &a, Some(true)).unwrap();
    let (addr, h) = listen_once(alice.0.clone());
    let (got, _, _) = mobilemesh_core::sync_shared(&st(&carol.0), &carol.1, &addr, &a).unwrap();
    h.join().unwrap().unwrap();
    assert_eq!(got, 0);
    assert!(sync::records(&st(&carol.0), &lista).unwrap().is_empty());
}

// ---------- Fase 13: hallazgos de la auditoría ----------

#[test]
fn contacto_no_puede_falsificar_numeros_ni_reloj() {
    let (alice, bob) = (device("alice18"), device("bob18"));
    befriend(&alice, "alice", &bob, "bob");
    let lista = "lista-alice".to_string();
    let (a, b) = (alice.1.id(), bob.1.id());
    let (da, db) = (to_hex_dev(&alice.1), to_hex_dev(&bob.1));
    st(&alice.0).set_share(&lista, &b, Some(true)).unwrap();
    st(&bob.0).set_share(&lista, &a, Some(true)).unwrap();
    sync::put_as(&st(&alice.0), &da, &a, Policy::Lww, &lista, "i1", "texto", Some("pan")).unwrap();
    let sync_bob = || {
        let (addr, h) = listen_once(alice.0.clone());
        let _ = mobilemesh_core::sync_shared(&st(&bob.0), &bob.1, &addr, &a);
        let _ = h.join();
    };
    sync_bob();
    // Bob fabrica: una operación con el dispositivo de Alice y un número enorme, y otra con el reloj
    // al máximo para ganar siempre.
    let base = sync::Op {
        device: da.clone(), seq: i64::MAX as u64, collection: lista.clone(), record: "i9".into(), field: "texto".into(),
        value: Some("trampa".into()), hlc: sync::now_hlc(), base: 0, policy: Policy::Lww, resolves: vec![], author: b.clone(),
    };
    let reloj = sync::Op { device: db.clone(), seq: 999, record: "i1".into(), value: Some("para siempre".into()), hlc: u64::MAX, ..base.clone() };
    // Y un dispositivo que Alice aún no conoce, con un número enorme, para que nadie reciba los suyos.
    let ajeno = sync::Op { device: "e".repeat(64), seq: 999_999, record: "i8".into(), ..base.clone() };
    st(&bob.0).insert_op(&ajeno).unwrap();
    let bstore = st(&bob.0);
    bstore.insert_op(&base).unwrap();
    bstore.insert_op(&reloj).unwrap();
    sync_bob();
    // Alice sigue pudiendo escribir y su valor no queda bloqueado.
    sync::put_as(&st(&alice.0), &da, &a, Policy::Lww, &lista, "i1", "texto", Some("leche")).unwrap();
    let rows = sync::records(&st(&alice.0), &lista).unwrap();
    let textos: Vec<&str> = rows.iter().map(|r| r.fields[0].1.as_str()).collect();
    assert_eq!(textos, ["leche"], "ni la trampa ni el reloj envenenado entran");
}

// ---------- Avisos en frío: buzón de cartas en el relé ----------

#[test]
fn mensaje_para_quien_duerme_espera_en_el_buzon_y_vuelve_el_acuse() {
    let (alice, bob) = (device("alice19"), device("bob19"));
    let (relay, _) = start_relay();
    for d in [&alice, &bob] {
        st(&d.0).set_setting("rele", Some(&relay)).unwrap();
    }
    let route_of = |d: &(PathBuf, Identity)| format!("127.0.0.1:9|{}", st(&d.0).relay_route().unwrap().unwrap());
    st(&alice.0).add_contact("bob", &bob.1.id(), Some(&route_of(&bob))).unwrap();
    st(&bob.0).add_contact("alice", &alice.1.id(), Some(&route_of(&alice))).unwrap();

    // Una primera conversación en directo (abre el ratchet y apunta el dispositivo de cada uno).
    assert_eq!(send_one(&alice, &bob, &alice.1, "hola"), Ok(1));
    assert_eq!(send_one(&bob, &alice, &bob.1, "hola alice"), Ok(1));

    // Bob duerme (nadie escucha): el mensaje queda en su buzón.
    let store = st(&alice.0);
    store.queue(&bob.1.id(), "¿estás dormido?").unwrap();
    let sent = mobilemesh_core::send_or_deposit(&store, &alice.1, &bob.1.id(), &route_of(&bob)).unwrap();
    assert_eq!(sent, mobilemesh_core::Sent::Deposited(1));
    assert!(statuses(&alice.0).contains(&"en buzón:¿estás dormido?".to_string()));

    // Alguien deja basura y una carta falsa en el buzón de Bob.
    let bob_box = st(&bob.0).relay_route().unwrap().unwrap().rsplit_once('/').unwrap().1.to_string();
    mobilemesh_core::relay::deposit(&relay, &bob_box, "AAAA").unwrap();

    // Bob despierta y recoge: solo entra el mensaje de verdad.
    assert_eq!(mobilemesh_core::collect_letters(&st(&bob.0), &bob.1).unwrap(), 1);
    assert!(statuses(&bob.0).contains(&"guardado:¿estás dormido?".to_string()));
    assert_eq!(mobilemesh_core::collect_letters(&st(&bob.0), &bob.1).unwrap(), 0, "el buzón quedó vacío");

    // Alice recoge el acuse: su mensaje pasa a entregado.
    mobilemesh_core::collect_letters(&st(&alice.0), &alice.1).unwrap();
    assert!(statuses(&alice.0).contains(&"entregado:¿estás dormido?".to_string()));
}

#[test]
fn el_rele_avisa_sin_contenido_a_quien_tiene_ficha_de_avisos() {
    let dir = std::env::temp_dir().join(format!("mm-avisos-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("avisos.txt");
    let script = dir.join("avisar.sh");
    std::fs::write(&script, format!("#!/bin/sh\necho \"$1\" >> '{}'\n", log.display())).unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();

    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let relay = l.local_addr().unwrap().to_string();
    let mut r = mobilemesh_core::relay::Relay::default();
    r.notifier = Some(script.display().to_string());
    thread::spawn(move || r.serve(l));

    let bob = device("bob20");
    st(&bob.0).set_setting("rele", Some(&relay)).unwrap();
    let (_, secret) = st(&bob.0).relay().unwrap().unwrap();
    let mailbox = st(&bob.0).relay_route().unwrap().unwrap().rsplit_once('/').unwrap().1.to_string();

    // Ficha falsa o firma de otro: no se acepta.
    assert!(mobilemesh_core::relay::set_push_token(&relay, &secret, Some("mala ficha")).is_err());
    mobilemesh_core::relay::set_push_token(&relay, &secret, Some("ficha-de-bob:123")).unwrap();

    // Dos cartas seguidas = un solo aviso, solo con la ficha.
    mobilemesh_core::relay::deposit(&relay, &mailbox, "AAAA").unwrap();
    mobilemesh_core::relay::deposit(&relay, &mailbox, "BBBB").unwrap();
    thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "ficha-de-bob:123\n");

    // Sin ficha, no hay aviso.
    mobilemesh_core::relay::set_push_token(&relay, &secret, None).unwrap();
    std::fs::remove_file(&log).unwrap();
    thread::sleep(std::time::Duration::from_secs(21));
    mobilemesh_core::relay::deposit(&relay, &mailbox, "CCCC").unwrap();
    thread::sleep(std::time::Duration::from_millis(500));
    assert!(!log.exists());
}

#[test]
fn cartas_viejas_se_leen_aunque_despues_llegue_un_mensaje_directo() {
    let (alice, bob) = (device("alice21"), device("bob21"));
    let (relay, _) = start_relay();
    for d in [&alice, &bob] {
        st(&d.0).set_setting("rele", Some(&relay)).unwrap();
    }
    let route_of = |d: &(PathBuf, Identity)| format!("127.0.0.1:9|{}", st(&d.0).relay_route().unwrap().unwrap());
    st(&alice.0).add_contact("bob", &bob.1.id(), Some(&route_of(&bob))).unwrap();
    st(&bob.0).add_contact("alice", &alice.1.id(), Some(&route_of(&alice))).unwrap();
    assert_eq!(send_one(&alice, &bob, &alice.1, "hola"), Ok(1));

    let store = st(&alice.0);
    for t in ["carta 1", "carta 2"] {
        store.queue(&bob.1.id(), t).unwrap();
        mobilemesh_core::send_or_deposit(&store, &alice.1, &bob.1.id(), &route_of(&bob)).unwrap();
    }
    // Bob despierta y le llega primero un mensaje directo nuevo…
    assert_eq!(send_one(&alice, &bob, &alice.1, "directo"), Ok(1));
    // …y después recoge las cartas viejas.
    assert_eq!(mobilemesh_core::collect_letters(&st(&bob.0), &bob.1).unwrap(), 2);
}

#[test]
fn el_acuse_vuelve_aunque_el_destino_no_tenga_mi_buzon_apuntado() {
    let (alice, bob) = (device("alice22"), device("bob22"));
    let (relay, _) = start_relay();
    for d in [&alice, &bob] {
        st(&d.0).set_setting("rele", Some(&relay)).unwrap();
    }
    let bob_route = format!("127.0.0.1:9|{}", st(&bob.0).relay_route().unwrap().unwrap());
    st(&alice.0).add_contact("bob", &bob.1.id(), Some(&bob_route)).unwrap();
    st(&bob.0).add_contact("alice", &alice.1.id(), Some("127.0.0.1:9")).unwrap(); // sin relé
    assert_eq!(send_one(&alice, &bob, &alice.1, "hola"), Ok(1));
    let store = st(&alice.0);
    store.queue(&bob.1.id(), "por carta").unwrap();
    mobilemesh_core::send_or_deposit(&store, &alice.1, &bob.1.id(), &bob_route).unwrap();
    assert_eq!(mobilemesh_core::collect_letters(&st(&bob.0), &bob.1).unwrap(), 1);
    mobilemesh_core::collect_letters(&st(&alice.0), &alice.1).unwrap();
    assert!(statuses(&alice.0).contains(&"entregado:por carta".to_string()));
}
