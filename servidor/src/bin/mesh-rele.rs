// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Juan Manuel Peralta Chacon (GitHub: juanmanuel767)
//! `mesh-rele`: relé de MobileMesh que cualquiera puede ejecutar (en casa, en un servidor…).
//! Solo empalma conexiones cifradas: no puede leerlas, no guarda nada y no escribe registros.

use mobilemesh_servidor::Relay;
use std::{io::Write, net::TcpListener, process::exit};

/// Uso: `mesh-rele [ip:puerto] [--avisos <programa>] [--max <conexiones>] [--directorio <archivo>] [--limite <KB/s>] [--websocket <ip:puerto>] [--tls <cert.pem> <clave.pem>]`.
/// `--websocket`: también acepta navegadores (WebSocket) en ese puerto.
/// `--limite`: velocidad máxima del relé sumando todo, para no dejar sin internet su casa. El programa (opcional)
/// se ejecuta con la ficha de avisos del teléfono cuando le llega una carta; por ejemplo
/// `tools/avisar-fcm.sh`. `--max` cambia cuántas conexiones atiende a la vez (1024 por defecto).
/// `--estado <127.0.0.1:puerto>`: entrega los números del relé en JSON (para vigilar su uso), solo en local.
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
                // Candado (wss://) en el mismo puerto si hay certificado.
                let tls = args.iter().position(|a| a == "--tls").and_then(|i| Some((args.get(i + 1)?.clone(), args.get(i + 2)?.clone())));
                if tls.is_some() {
                    println!("con candado (wss://) en el mismo puerto");
                }
                std::thread::spawn(move || mobilemesh_servidor::websocket::serve(l, relay, tls));
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
    if let Some(dir) = args.iter().position(|a| a == "--estado").and_then(|i| args.get(i + 1)) {
        // Solo en local: el puente web llega como 127.0.0.1 al puerto del relé, pero no a este.
        let l = match TcpListener::bind(dir) {
            Ok(l) if l.local_addr().is_ok_and(|a| a.ip().is_loopback()) => l,
            _ => {
                eprintln!("--estado necesita una dirección local libre (127.0.0.1:puerto)");
                exit(1);
            }
        };
        println!("números del relé en {dir}");
        let (relay, inicio) = (relay.clone(), std::time::Instant::now());
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let json = relay.estado();
                // De una vez: quien lee no recibe la línea a trozos.
                let linea = format!("{},\"encendido\":{}}}\n", &json[..json.len() - 1], inicio.elapsed().as_secs());
                let _ = s.write_all(linea.as_bytes());
            }
        });
    }
    relay.serve(listener);
}
