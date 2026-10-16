//! `mesh-mcp`: servidor MCP (Model Context Protocol) para que cualquier IA (Claude, ChatGPT,
//! Codex…) use MobileMesh: leer la guía del SDK, crear nodos de prueba, guardar y consultar datos,
//! añadir contactos, dar permisos y comprobar la red. Habla JSON-RPC por la entrada/salida estándar.
//!
//! Alta en Claude Code:  claude mcp add mobilemesh -- /ruta/a/mesh-mcp
//!
//! Los nodos de prueba viven en carpetas que elige la IA; nada sale del equipo salvo `estado_red`,
//! que pregunta a los relés y guardianes públicos si responden.

use mobilemesh_core::{
    Identity, Store, directorio,
    identity::{Plain, to_hex},
    parse_invite, relay,
    sync::{self, Policy},
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    time::Duration,
};

const GUIA: &str = include_str!("../../../docs/sdk.md");

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // notificaciones: sin respuesta
        let reply = match msg["method"].as_str().unwrap_or("") {
            "initialize" => json!({"jsonrpc": "2.0", "id": id, "result": {
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mobilemesh", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "MobileMesh es un backend local-first y P2P: los datos viven cifrados en cada teléfono y se sincronizan sin servidor central. Empieza por la herramienta `guia` para integrarlo en una app Android."
            }}),
            "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
            "tools/list" => json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tools()}}),
            "tools/call" => {
                let (text, error) = match call(msg["params"]["name"].as_str().unwrap_or(""), &msg["params"]["arguments"]) {
                    Ok(t) => (t, false),
                    Err(e) => (e.to_string(), true),
                };
                json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": text}], "isError": error}})
            }
            m => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("método desconocido: {m}")}}),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({"name": name, "description": description, "inputSchema": {"type": "object", "properties": props, "required": required}})
}

fn s(description: &str) -> Value {
    json!({"type": "string", "description": description})
}

fn tools() -> Value {
    let carpeta = s("Nombre del nodo de prueba (carpeta dentro de ~/.mobilemesh-mcp), p. ej. «ana»");
    json!([
        tool("guia", "Guía completa del SDK de Android de MobileMesh (Kotlin): cuenta, colecciones, permisos, archivos, mensajes, ayudantes, relés y recuperación.", json!({}), &[]),
        tool("crear_nodo", "Crea un nodo de prueba (identidad y base de datos cifrada) en una carpeta.", json!({"carpeta": carpeta}), &["carpeta"]),
        tool("guardar_dato", "Guarda un campo de un registro en una colección del nodo (se cifra y se sincroniza con quien la comparta).",
            json!({"carpeta": carpeta, "coleccion": s("Nombre de la colección"), "registro": s("Id del registro"), "campo": s("Nombre del campo"), "valor": s("Valor (texto)")}),
            &["carpeta", "coleccion", "registro", "campo", "valor"]),
        tool("ver_datos", "Lista los registros de una colección del nodo con sus campos.", json!({"carpeta": carpeta, "coleccion": s("Nombre de la colección")}), &["carpeta", "coleccion"]),
        tool("invitacion", "Invitación del nodo (lo que va en el QR) para que otro lo añada como contacto.", json!({"carpeta": carpeta, "direccion": s("ip:puerto donde escucha el nodo")}), &["carpeta", "direccion"]),
        tool("anadir_contacto", "Añade un contacto de confianza con su invitación.", json!({"carpeta": carpeta, "nombre": s("Nombre del contacto"), "invitacion": s("mobilemesh:2:…")}), &["carpeta", "nombre", "invitacion"]),
        tool("dar_permiso", "Comparte una colección con un contacto: leer, escribir o nada (quitar). Se comprueba en cada cambio que llega.",
            json!({"carpeta": carpeta, "coleccion": s("Nombre de la colección"), "contacto": s("Nombre del contacto"), "permiso": {"type": "string", "enum": ["leer", "escribir", "nada"]}}),
            &["carpeta", "coleccion", "contacto", "permiso"]),
        tool("estado_red", "Relés y guardianes públicos (del directorio firmado) y si responden ahora.", json!({}), &[]),
    ])
}

fn arg<'a>(a: &'a Value, k: &str) -> mobilemesh_core::Result<&'a str> {
    a.get(k).and_then(Value::as_str).ok_or_else(|| format!("falta «{k}»").into())
}

/// Los nodos solo pueden vivir dentro de la carpeta del servidor (`MESH_MCP_DIR`, por defecto
/// `~/.mobilemesh-mcp`): la IA no puede crear archivos en cualquier sitio del equipo.
fn carpeta(nombre: &str) -> mobilemesh_core::Result<std::path::PathBuf> {
    use std::path::Component;
    let p = Path::new(nombre);
    if nombre.is_empty() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err("la carpeta debe ser un nombre simple, sin «/» al principio ni «..»".into());
    }
    let base = std::env::var_os("MESH_MCP_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".mobilemesh-mcp")))
        .ok_or("no sé dónde guardar los nodos: define MESH_MCP_DIR")?;
    Ok(base.join(p))
}

fn open(dir: &str) -> mobilemesh_core::Result<(Identity, Store)> {
    let dir = carpeta(dir)?;
    let me = Identity::load(&dir, &Plain).map_err(|_| "esa carpeta no tiene nodo: usa `crear_nodo`")?;
    let store = Store::open(&dir, me.db_key())?;
    Ok((me, store))
}

fn call(name: &str, a: &Value) -> mobilemesh_core::Result<String> {
    Ok(match name {
        "guia" => GUIA.to_string(),
        "crear_nodo" => {
            let dir = carpeta(arg(a, "carpeta")?)?;
            let me = Identity::create(&dir, &Plain)?;
            Store::open(&dir, me.db_key())?;
            format!("nodo creado en {}; cuenta {}", dir.display(), me.id())
        }
        "guardar_dato" => {
            let (me, store) = open(arg(a, "carpeta")?)?;
            let (c, r, f, v) = (arg(a, "coleccion")?, arg(a, "registro")?, arg(a, "campo")?, arg(a, "valor")?);
            sync::put_as(&store, &to_hex(&me.public), &me.id(), Policy::Lww, c, r, f, Some(v))?;
            format!("guardado {c}/{r}.{f}")
        }
        "ver_datos" => {
            let (_, store) = open(arg(a, "carpeta")?)?;
            let rows: Vec<Value> = sync::records(&store, arg(a, "coleccion")?)?
                .into_iter()
                .map(|r| json!({"id": r.id, "campos": r.fields.into_iter().map(|(k, v)| (k, Value::String(v))).collect::<serde_json::Map<String, Value>>()}))
                .collect();
            serde_json::to_string_pretty(&rows)?
        }
        "invitacion" => {
            let (me, store) = open(arg(a, "carpeta")?)?;
            let dir = arg(a, "direccion")?;
            let route = store.relay_route()?.map_or(dir.to_string(), |r| format!("{dir}|{r}"));
            mobilemesh_core::invite(&me, &route)
        }
        "anadir_contacto" => {
            let (_, store) = open(arg(a, "carpeta")?)?;
            let (key, addr) = parse_invite(arg(a, "invitacion")?)?;
            store.add_contact(arg(a, "nombre")?, &key, Some(&addr))?;
            format!("contacto «{}» añadido", arg(a, "nombre")?)
        }
        "dar_permiso" => {
            let (_, store) = open(arg(a, "carpeta")?)?;
            let key = store.contact_key(arg(a, "contacto")?)?.ok_or("contacto desconocido")?;
            let access = match arg(a, "permiso")? {
                "leer" => Some(false),
                "escribir" => Some(true),
                "nada" => None,
                _ => return Err("permiso: leer | escribir | nada".into()),
            };
            store.set_share(arg(a, "coleccion")?, &key, access)?;
            "permiso aplicado".into()
        }
        "estado_red" => {
            let fijos: Vec<String> = relay::UNIVERSALES.iter().map(|r| r.to_string()).collect();
            let (reles, guardianes, v) = match directorio::descargar(&fijos) {
                Some((d, _)) => (d.reles, d.guardianes, format!("directorio firmado versión {}", d.v)),
                None => (fijos, mobilemesh_core::guardian::UNIVERSALES.iter().map(|g| g.to_string()).collect(), "sin directorio: listas fijas".into()),
            };
            let mut lines = vec![v];
            for r in &reles {
                lines.push(format!("relé {r}: {}", if relay::public_ip(r).is_ok() { "responde" } else { "NO responde" }));
            }
            for g in &guardianes {
                let ok = parse_invite(g).ok().and_then(|(_, route)| route.to_socket_addrs().ok()?.next())
                    .is_some_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(5)).is_ok());
                lines.push(format!("guardián {}: {}", g.rsplit(':').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(":"), if ok { "responde" } else { "NO responde" }));
            }
            lines.join("\n")
        }
        _ => return Err(format!("herramienta desconocida: {name}").into()),
    })
}
