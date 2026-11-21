//! Tope de partes de una ruta.
//!
//! Una ruta la elige quien llama, y `Session::connect` lanza un hilo por parte esperando
//! `VENTAJA * i` antes de cada una. Sin tope, una llamada con miles de partes retiene durante
//! minutos el bucle que la usa (por ejemplo el de un ayudante abierto, que atiende a todos en
//! un solo hilo). Hallazgo de la auditoría de seguridad del 20-11-2026.

use mobilemesh_core::transport::{MAX_PARTES_RUTA, check_route};

/// Una ruta como las que genera el propio proyecto tiene que seguir valiendo: el supernodo
/// elegido, su dirección de la wifi, los dos relés de `relay::choose` y las directas IPv4 e IPv6.
#[test]
fn una_ruta_real_con_seis_partes_sigue_valiendo() {
    let buzon = "0".repeat(32);
    let ruta = format!(
        "192.168.20.7:7850|[2800:484:9f77:1600:fba8:46e4:289d:1a4f]:7850|relay=181.60.197.125:7843/{buzon}|relay=192.168.20.19:7840/{buzon}|relay=203.0.113.9:7900/{buzon}|relay=198.51.100.4:7900/{buzon}"
    );
    assert_eq!(ruta.split('|').count(), 6, "la ruta de ejemplo debe tener seis partes");
    assert!(check_route(&ruta).is_ok(), "una ruta legítima de seis partes no puede rechazarse: {ruta}");
}

/// Justo en el tope se acepta, y una más se rechaza.
#[test]
fn el_tope_de_partes_se_respeta_exactamente() {
    let parte = "203.0.113.9:7900";
    let justas = vec![parte; MAX_PARTES_RUTA].join("|");
    assert!(check_route(&justas).is_ok(), "{MAX_PARTES_RUTA} partes están dentro del tope");

    let una_mas = vec![parte; MAX_PARTES_RUTA + 1].join("|");
    assert!(check_route(&una_mas).is_err(), "{} partes deben rechazarse", MAX_PARTES_RUTA + 1);
}

/// El caso del hallazgo: miles de partes se rechazan al instante, sin lanzar un hilo por parte.
#[test]
fn una_ruta_de_miles_de_partes_se_rechaza() {
    let ruta = vec!["10.255.255.1:9"; 4000].join("|");
    let error = check_route(&ruta).expect_err("4000 partes deben rechazarse");
    assert!(
        error.to_string().contains("demasiadas partes"),
        "el error debe explicar el motivo, no fallar por otra razón: {error}"
    );
}
