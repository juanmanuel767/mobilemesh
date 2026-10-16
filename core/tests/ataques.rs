//! Fase 13: ataques simulados. Ninguno debe colgar, romper (pánico) ni colar nada.

use mobilemesh_core::{Identity, Store, identity::Plain, serve};
use mobilemesh_servidor::Relay;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

/// Las pruebas que usan los límites de conexión (comunes a todo el programa) van de una en una.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn device(name: &str) -> (PathBuf, Identity) {
    let dir = std::env::temp_dir().join(format!("mm-at-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let id = Identity::create(&dir, &Plain).unwrap();
    (dir, id)
}

/// Lanza `serve` sobre una conexión y devuelve (dirección, resultado que dio `serve` y cuánto tardó).
fn victim(tag: &str) -> (String, thread::JoinHandle<(bool, Duration)>) {
    let (dir, me) = device(tag);
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let h = thread::spawn(move || {
        let (s, _) = l.accept().unwrap();
        let t = Instant::now();
        let store = Store::open(&dir, me.db_key()).unwrap();
        let ok = serve(&store, &me, s).is_ok();
        (ok, t.elapsed())
    });
    (addr, h)
}

#[test]
fn basura_en_vez_del_saludo() {
    let (addr, h) = victim("basura");
    let mut s = TcpStream::connect(&addr).unwrap();
    let junk: Vec<u8> = (0..5000u32).map(|i| (i * 7919 % 251) as u8).collect();
    let _ = s.write_all(&junk);
    drop(s);
    let (ok, _) = h.join().expect("no debe haber pánico");
    assert!(!ok, "la basura se rechaza");
}

#[test]
fn longitud_enorme_y_corte() {
    let (addr, h) = victim("enorme");
    let mut s = TcpStream::connect(&addr).unwrap();
    s.write_all(&[0xFF, 0xFF, 1, 2, 3]).unwrap(); // anuncia 65 535 bytes y manda 3
    drop(s);
    let (ok, _) = h.join().expect("no debe haber pánico");
    assert!(!ok);
}

#[test]
fn conexion_a_medias_no_bloquea_para_siempre() {
    let (addr, h) = victim("lento");
    let mut s = TcpStream::connect(&addr).unwrap();
    s.write_all(&[0, 32]).unwrap(); // promete 32 bytes y se queda callado
    let (ok, took) = h.join().expect("no debe haber pánico");
    drop(s);
    assert!(!ok);
    assert!(took < Duration::from_secs(20), "se libera por el plazo total del saludo ({took:?})");
}

#[test]
fn saludo_valido_de_un_desconocido_no_obtiene_nada() {
    let (addr, h) = victim("desconocido");
    let (_, stranger) = device("extrano");
    let r = mobilemesh_core::transport::Session::connect(&addr, &stranger, &[0u8; 32]);
    assert!(r.is_err(), "no sabe la cuenta del otro: ni siquiera completa el saludo");
    let (ok, _) = h.join().expect("no debe haber pánico");
    assert!(!ok);
}

#[test]
fn rele_aguanta_lineas_raras_y_avalanchas() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let relay = Relay::default();
    let r = relay.clone();
    thread::spawn(move || r.serve(l));
    // Línea sin fin, órdenes inventadas, secreto corto, buzón que no existe.
    for msg in [vec![b'A'; 10_000], b"ROBAR todo\n".to_vec(), b"REGISTRAR corto\n".to_vec(), b"MARCAR nadie\n".to_vec()] {
        let mut s = TcpStream::connect(&addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(12))).unwrap();
        let _ = s.write_all(&msg);
        let mut buf = [0u8; 64];
        let n = s.read(&mut buf).unwrap_or(0);
        let answer = String::from_utf8_lossy(&buf[..n]);
        assert!(!answer.starts_with("OK"), "no se concede nada: {answer:?}");
    }
    // Registrar un buzón ajeno sin poder firmar el reto no sirve.
    let mut s = TcpStream::connect(&addr).unwrap();
    // Clave pública válida de otra persona (el atacante la conoce, pero no su clave privada).
    let ajena = ed25519_dalek::SigningKey::from_bytes(&[5u8; 32]).verifying_key().to_bytes();
    let ajena: String = ajena.iter().map(|b| format!("{b:02x}")).collect();
    s.write_all(format!("REGISTRAR {ajena}\n").as_bytes()).unwrap();
    let mut buf = [0u8; 128];
    let n = s.read(&mut buf).unwrap_or(0);
    assert!(String::from_utf8_lossy(&buf[..n]).starts_with("RETO "));
    s.write_all(format!("FIRMA {}\n", "00".repeat(64)).as_bytes()).unwrap();
    let n = s.read(&mut buf).unwrap_or(0);
    assert!(!String::from_utf8_lossy(&buf[..n]).starts_with("OK"), "sin la clave no hay buzón");
}

#[test]
fn invitaciones_y_codigos_raros_no_rompen_nada() {
    for s in ["", "mobilemesh:2:", "mobilemesh:2:zz:1.2.3.4:5", &format!("mobilemesh:2:{}:", "a".repeat(64)),
              &format!("mobilemesh:2:{}:relay=x/y", "a".repeat(64)), "mobilemesh:2:ñññ", &"x".repeat(100_000)] {
        assert!(mobilemesh_core::parse_invite(s).is_err(), "{:.40}", s);
    }
    for c in ["", "MM2-", "MM2-zz", &format!("MM2-{}", "0".repeat(151)), "MM1-00"] {
        assert!(mobilemesh_core::recovery::import(c, "contraseña larga").is_err());
    }
    assert!(mobilemesh_core::access::verify("{}", &"a".repeat(64)).is_err());
    assert!(mobilemesh_core::access::verify("no es json", &"a".repeat(64)).is_err());
}

#[test]
fn una_conexion_lenta_no_bloquea_a_las_demas() {
    let _uno_a_uno = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (dir, me) = device("ocupado");
    let (adir, alice) = device("alice-ocupado");
    Store::open(&dir, me.db_key()).unwrap().add_contact("alice", &alice.id(), None).unwrap();
    Store::open(&adir, alice.db_key()).unwrap().add_contact("bob", &me.id(), None).unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    thread::spawn(move || mobilemesh_core::accept_loop(l, dir, me.clone(), || false, |_| {}));
    // Varias conexiones que se quedan calladas a medias (desde la misma dirección que el usuario de
    // la prueba, así que menos que el límite por dirección)…
    let _lentas: Vec<TcpStream> = (0..mobilemesh_core::MAX_PER_IP - 1).map(|_| {
        let mut s = TcpStream::connect(&addr).unwrap();
        s.write_all(&[0, 32]).unwrap();
        s
    }).collect();
    // …y aun así Alice entrega al momento.
    let store = Store::open(&adir, alice.db_key()).unwrap();
    let bob = store.contact_key("bob").unwrap().unwrap();
    store.queue(&bob, "sigo llegando").unwrap();
    let t = Instant::now();
    assert_eq!(mobilemesh_core::deliver(&store, &alice, &bob, &addr).unwrap(), 1);
    assert!(t.elapsed() < Duration::from_secs(5), "tardó {:?}", t.elapsed());
}

#[test]
fn una_misma_direccion_no_acapara_las_conexiones() {
    let _uno_a_uno = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (dir, me) = device("acaparar");
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    thread::spawn(move || mobilemesh_core::accept_loop(l, dir, me, || false, |_| {}));
    let _lentas: Vec<TcpStream> = (0..mobilemesh_core::MAX_PER_IP).map(|_| {
        let mut s = TcpStream::connect(&addr).unwrap();
        s.write_all(&[0, 32]).unwrap();
        s
    }).collect();
    thread::sleep(Duration::from_millis(300));
    // La siguiente desde la misma dirección se cierra al momento, sin contestar.
    let mut otra = TcpStream::connect(&addr).unwrap();
    otra.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut b = [0u8; 1];
    let t = Instant::now();
    assert!(matches!(otra.read(&mut b), Ok(0) | Err(_)));
    assert!(t.elapsed() < Duration::from_secs(3), "se cierra enseguida");
}
