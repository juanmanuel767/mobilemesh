//! `mesh-rele`: relé de MobileMesh que cualquiera puede ejecutar (en casa, en un servidor…).
//! Solo empalma conexiones cifradas: no puede leerlas, no guarda nada y no escribe registros.

use mobilemesh_core::relay::Relay;
use std::{net::TcpListener, process::exit};

/// Uso: `mesh-rele [ip:puerto] [--avisos <programa>]`. El programa (opcional) se ejecuta con la
/// ficha de avisos del teléfono cuando le llega una carta; por ejemplo `tools/avisar-fcm.sh`.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let notifier = args.iter().position(|a| a == "--avisos").and_then(|i| args.get(i + 1)).cloned();
    let addr = args.first().filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "0.0.0.0:7900".into());
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("no se pudo abrir {addr}: {e}");
        exit(1);
    });
    println!("relé de MobileMesh escuchando en {addr} (no guarda nada ni escribe registros)");
    if let Some(n) = &notifier {
        println!("avisos activados con {n}");
    }
    let mut relay = Relay::default();
    relay.notifier = notifier;
    relay.serve(listener);
}
