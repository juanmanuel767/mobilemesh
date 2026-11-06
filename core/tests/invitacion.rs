//! Invitaciones que la propia app genera y que otra app debe aceptar.
#![allow(clippy::unwrap_used)]

use mobilemesh_core::parse_invite;

/// Invitación real tomada de un Redmi A5 el 6 de noviembre de 2026, con los
/// tres relés que la app añade cuando hay un supernodo en la misma red.
const REAL: &str = "mobilemesh:2:a64d28bcd7a2aeef3bfec78b22ebc1650c4d5a6bb2c13fba3a9f21bddc3837cd:192.168.20.7:7801|relay=181.60.197.125:7841/bbc1168cd2dc3414d693f8353164becf|relay=192.168.20.19:7841/bbc1168cd2dc3414d693f8353164becf|relay=mobilemeshjuan.duckdns.org:7900/bbc1168cd2dc3414d693f8353164becf";

#[test]
fn una_invitacion_de_la_propia_app_se_acepta() {
    match parse_invite(REAL) {
        Ok((key, addr)) => {
            assert_eq!(key.len(), 64, "la clave debería tener 64 caracteres");
            assert!(addr.starts_with("192.168.20.7:7801"), "ruta inesperada: {addr}");
        }
        Err(e) => panic!("la app rechaza una invitación que ella misma genera: {e}"),
    }
}

/// Un desconocido rechaza en silencio (decisión de `docs/fase-0.md`), así que
/// quien envía solo recibe un cierre seco de la conexión. Esta prueba fija que
/// el mensaje que ve el usuario incluya la causa más probable, porque sin ella
/// el fallo es indistinguible de un problema de red.
#[test]
fn el_rechazo_de_un_desconocido_explica_la_causa_probable() {
    use mobilemesh_core::{Identity, Store, identity::Plain, send_or_deposit};

    let dir = std::env::temp_dir().join(format!("mm-inv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let yo = Identity::create(&dir, &Plain).unwrap();
    let store = Store::open(&dir, yo.db_key()).unwrap();

    // Un destinatario que no existe: nadie escucha en ese puerto.
    let ajeno = "b".repeat(64);
    store.add_contact("fantasma", &ajeno, Some("127.0.0.1:1")).unwrap();
    store.queue(&ajeno, "hola").unwrap();

    let error = match send_or_deposit(&store, &yo, &ajeno, "127.0.0.1:1") {
        Ok(_) => panic!("no debería entregarse a un puerto donde nadie escucha"),
        Err(e) => e.to_string(),
    };
    // Aquí nadie escucha, así que el fallo es «connection refused», no un
    // cierre: la pista NO debe aparecer, para no sugerir una causa falsa.
    assert!(
        !error.contains("todavía no te tenga añadido"),
        "la pista no debe añadirse a un fallo que no es un cierre: {error}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
