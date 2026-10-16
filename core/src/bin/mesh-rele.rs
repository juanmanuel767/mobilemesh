//! `mesh-rele`: relé de MobileMesh que cualquiera puede ejecutar (en casa, en un servidor…).
//! Solo empalma conexiones cifradas: no puede leerlas, no guarda nada y no escribe registros.

use mobilemesh_core::relay::Relay;
use std::{net::TcpListener, process::exit};

/// Uso: `mesh-rele [ip:puerto] [--avisos <programa>] [--max <conexiones>] [--directorio <archivo>] [--limite <KB/s>] [--websocket <ip:puerto>]`.
/// `--websocket`: también acepta navegadores (WebSocket) en ese puerto.
/// `--limite`: velocidad máxima del relé sumando todo, para no dejar sin internet su casa. El programa (opcional)
/// se ejecuta con la ficha de avisos del teléfono cuando le llega una carta; por ejemplo
/// `tools/avisar-fcm.sh`. `--max` cambia cuántas conexiones atiende a la vez (1024 por defecto).
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let notifier = args.iter().position(|a| a == "--avisos").and_then(|i| args.get(i + 1)).cloned();
    let max = args.iter().position(|a| a == "--max").and_then(|i| args.get(i + 1)).map(|n| {
        n.parse::<usize>().unwrap_or_else(|_| {
            eprintln!("--max necesita un número");
            exit(1);
        })
    });
    let addr = args.first().filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "0.0.0.0:7900".into());
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("no se pudo abrir {addr}: {e}");
        exit(1);
    });
    println!("relé de MobileMesh escuchando en {addr} (no guarda nada ni escribe registros)");
    // Puente para navegadores: WebSocket en otro puerto, empalmado con este mismo relé.
    if let Some(ws) = args.iter().position(|a| a == "--websocket").and_then(|i| args.get(i + 1)) {
        let relay = listener.local_addr().map(|a| std::net::SocketAddr::from(([127, 0, 0, 1], a.port())));
        match (TcpListener::bind(ws), relay) {
            (Ok(l), Ok(relay)) => {
                println!("navegadores (WebSocket) en {ws}");
                std::thread::spawn(move || mobilemesh_core::websocket::serve(l, relay));
            }
            _ => {
                eprintln!("no se pudo abrir {ws} para WebSocket");
                exit(1);
            }
        }
    }
    if let Some(n) = &notifier {
        println!("avisos activados con {n}");
    }
    let mut relay = Relay::default();
    relay.notifier = notifier;
    relay.max = max;
    relay.limite = args.iter().position(|a| a == "--limite").and_then(|i| args.get(i + 1)).map(|n| {
        n.parse::<u64>().map(|kb| kb * 1024).unwrap_or_else(|_| {
            eprintln!("--limite necesita un número (KB por segundo)");
            exit(1);
        })
    });
    // Directorio firmado de relés y guardianes que se entrega a las apps (se lee en cada petición).
    relay.directorio = args.iter().position(|a| a == "--directorio").and_then(|i| args.get(i + 1)).cloned();
    relay.serve(listener);
}
