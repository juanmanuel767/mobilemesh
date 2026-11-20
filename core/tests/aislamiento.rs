//! Aislamiento por aplicación: dos apps distintas comparten relés y supernodos, pero no pueden
//! verse ni hablarse. El App ID entra en la derivación del buzón y en el prólogo del saludo, así
//! que la separación es criptográfica: no es un campo que se pueda falsificar.

use mobilemesh_core::{Identity, Store, deliver, identity::Plain, relay, serve};
use std::{net::TcpListener, path::PathBuf, thread};

const APP_A: &str = "com.app.a";
const APP_B: &str = "com.app.b";

fn device(name: &str, app: &str) -> (PathBuf, Identity) {
    let dir = std::env::temp_dir().join(format!("mm-aisl-{}-{name}-{:?}", std::process::id(), thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let id = Identity::create(&dir, &Plain).unwrap();
    let store = Store::open(&dir, id.db_key()).unwrap();
    store.set_setting("app", Some(app)).unwrap();
    (dir, id)
}

fn st(dir: &std::path::Path) -> Store {
    Store::open(dir, Identity::load(dir, &Plain).unwrap().db_key()).unwrap()
}

/// Dispositivo que atiende UNA conexión; devuelve su dirección y el resultado de atenderla.
fn listen_once(dir: PathBuf) -> (String, thread::JoinHandle<Result<usize, String>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let me = Identity::load(&dir, &Plain).unwrap();
        serve(&st(&dir), &me, l.accept().unwrap().0).map_err(|e| e.to_string())
    });
    (addr, h)
}

/// Se conocen entre sí (son contactos): aun así, si son de apps distintas no deben hablarse.
fn befriend(a: &(PathBuf, Identity), b: &(PathBuf, Identity)) {
    st(&a.0).add_contact("otro", &b.1.id(), None).unwrap();
    st(&b.0).add_contact("otro", &a.1.id(), None).unwrap();
}

#[test]
fn dos_apps_distintas_no_completan_el_saludo() {
    let (ana, beto) = (device("ana", APP_A), device("beto", APP_B));
    befriend(&ana, &beto);
    st(&ana.0).queue(&beto.1.id(), "hola desde la app A").unwrap();

    let (addr, h) = listen_once(beto.0.clone());
    let r = deliver(&st(&ana.0), &ana.1, &beto.1.id(), &addr);
    let _ = h.join();

    assert!(r.is_err(), "una app no debe poder entregar nada a otra app");
    // Y el mensaje sigue sin entregar en el origen.
    assert!(!st(&ana.0).list().unwrap().is_empty(), "el mensaje se queda pendiente");
}

#[test]
fn la_misma_app_si_se_habla() {
    let (ana, beto) = (device("ana-ok", APP_A), device("beto-ok", APP_A));
    befriend(&ana, &beto);
    st(&ana.0).queue(&beto.1.id(), "hola vecina").unwrap();

    let (addr, h) = listen_once(beto.0.clone());
    let enviados = deliver(&st(&ana.0), &ana.1, &beto.1.id(), &addr).expect("misma app: debe entregar");
    let recibidos = h.join().unwrap().expect("el destino atiende");

    assert_eq!((enviados, recibidos), (1, 1));
    assert!(st(&beto.0).list().unwrap().iter().any(|m| m.body == "hola vecina"));
}

#[test]
fn el_buzon_del_rele_es_distinto_en_cada_app() {
    // Mismo teléfono y mismo secreto de buzón: el relé ve dos buzones sin relación.
    let secreto = "secreto-de-buzon-de-prueba";
    let (a, b) = (relay::mailbox_id(secreto, APP_A), relay::mailbox_id(secreto, APP_B));
    assert_ne!(a, b, "dos apps no pueden compartir buzón");
    assert_eq!(a, relay::mailbox_id(secreto, APP_A), "el buzón de una app es estable");
    assert_eq!(a.len(), 32, "el relé sigue viendo un identificador opaco de 32 hex");
}

#[test]
fn una_invitacion_de_otra_app_se_rechaza() {
    let (ana, _) = (device("ana-inv", APP_A), ());
    let yo = Identity::load(&ana.0, &Plain).unwrap();
    let inv = mobilemesh_core::invite(&yo, APP_A, "127.0.0.1:7900");

    let (_, _, app) = mobilemesh_core::parse_invite(&inv, APP_A).expect("mi propia app la acepta");
    assert_eq!(app, APP_A);

    let err = mobilemesh_core::parse_invite(&inv, APP_B).unwrap_err().to_string();
    assert!(err.contains("otra aplicación"), "error inesperado: {err}");
}

#[test]
fn una_invitacion_antigua_se_rechaza() {
    let vieja = "mobilemesh:2:aabbccddeeff00112233445566778899aabbccddeeff001122334455667788:127.0.0.1:7900";
    let err = mobilemesh_core::parse_invite(vieja, APP_A).unwrap_err().to_string();
    assert!(err.contains("versión anterior"), "error inesperado: {err}");
}
