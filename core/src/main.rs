//! `mesh`: herramienta de pruebas para simular dispositivos desde la terminal.

use mobilemesh_core::{
    Identity, Pending, Result, Store, approve_link, complete_link, deliver, fingerprint,
    identity::{Plain, Stored, load_any},
    invite, link_request, parse_invite, recovery, serve,
};
use std::{env, io::stdin, net::TcpListener, path::Path, process::exit};

const USO: &str = "uso:
  mesh <carpeta> init                               crear identidad y base de datos
  mesh <carpeta> id                                 ver mi clave pública (para compartir)
  mesh <carpeta> invitacion <ip:puerto>             ver mi invitación (lo que va en el QR)
  mesh <carpeta> contacto <nombre> <clave> [ip:puerto]
  mesh <carpeta> contacto <nombre> <invitación>     añadir un contacto de confianza
  mesh <carpeta> escuchar <ip:puerto>               recibir mensajes
  mesh <carpeta> ayudante <ip:puerto> <programa>    funciones de servidor: atiende llamadas de
                                                    cualquiera ejecutando «<programa> <función>»
                                                    (recibe {de, datos} por la entrada y
                                                    devuelve el resultado por la salida)
  mesh <carpeta> enviar <nombre> <ip:puerto> <texto>
  mesh <carpeta> reintentar <nombre> <ip:puerto>    reenviar los pendientes
  mesh <carpeta> mensajes                           ver mensajes y su estado
  mesh <carpeta> archivo <nombre> <ip:puerto> <ruta>   enviar un archivo
  mesh <carpeta> exportar <id> <destino>            descifrar un archivo recibido o enviado
  mesh <carpeta> aceptar-copias <nombre>            guardar copias cifradas de un contacto
  mesh <carpeta> copias                             ver las copias que guardo para otros
  mesh <carpeta> usar-rele <ip:puerto>              recibir también a través de un relé
  mesh <carpeta> buzon                              recoger mensajes y acuses dejados en mi buzón
  mesh <carpeta> sincronizar <ruta>                 sincronizar con otro dispositivo de mi cuenta
  mesh <carpeta> compartir <colección> <nombre> leer|escribir|nada
  mesh <carpeta> sincronizar-con <nombre> <ruta>    sincronizar lo compartido con un contacto
  mesh <carpeta> dato <colección> <registro> <campo> <valor>
  mesh <carpeta> datos <colección>                  ver una colección
  mesh <carpeta> copia                              crear copia de recuperación (pide contraseña)
  mesh <carpeta> directorio                         buscar en mis relés relés y guardianes nuevos
  mesh <carpeta> guardian <ip:puerto> <ruta pública>   ser guardián: guardar códigos y copias cifradas de cualquiera
  mesh <carpeta> nombres <ip:puerto> <ruta pública>    ser guía de nombres: registrar y buscar usuarios por nombre
  mesh <carpeta> registrar-nombre <guía> <nombre> <mi ruta>   registrar mi nombre en una guía
  mesh <carpeta> buscar-nombre <guía> <nombre>        invitación de un nombre
  mesh <carpeta> pedir-contacto <guía> <nombre>       avisar a un nombre de que quiero hablar con él
  mesh <carpeta> solicitudes <guía>                   añadir como contactos a quienes quieren hablar conmigo
  mesh <carpeta> guardar-cuenta <invitación> <usuario> guardar mi código en un guardián (pide contraseña)
  mesh <carpeta> buscar-cuenta <invitación> <usuario>  teléfono nuevo: recuperar la cuenta desde un guardián
  mesh <carpeta> restaurar <código>                 recuperar la cuenta (pide contraseña)
  mesh <carpeta> vincular-pedir                     dispositivo nuevo: mostrar su solicitud
  mesh <carpeta> vincular-aprobar <solicitud>       principal: aprobarla y mostrar el certificado
  mesh <carpeta> vincular-completar <certificado>   dispositivo nuevo: instalar el certificado";

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        exit(1);
    }
}

/// Lee la contraseña de la entrada estándar, para que no quede en el historial de la terminal.
fn password() -> Result<String> {
    eprintln!("contraseña:");
    let mut s = String::new();
    stdin().read_line(&mut s)?;
    Ok(s.trim_end_matches(['\r', '\n']).to_string())
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let Some((dir, cmd)) = args.split_first() else { return Err(USO.into()) };
    let dir = Path::new(dir);
    let open = || -> Result<(Identity, Store)> {
        let me = Identity::load(dir, &Plain)?;
        let store = Store::open(dir, me.db_key())?;
        Ok((me, store))
    };
    match cmd {
        ["init"] => {
            let me = Identity::create(dir, &Plain)?;
            Store::open(dir, me.db_key())?;
            println!("identidad creada: {}", me.id());
        }
        ["id"] => println!("{}", Identity::load(dir, &Plain)?.id()),
        ["vincular-pedir"] => {
            let p = Pending::create(dir, &Plain)?;
            println!("{}\ncódigo de comprobación: {}", link_request(&p), fingerprint(&p.public));
        }
        ["vincular-aprobar", request] => println!("{}", approve_link(&Identity::load(dir, &Plain)?, request)?),
        ["vincular-completar", answer] => {
            let Stored::Pending(p) = load_any(dir, &Plain)? else { return Err("esta carpeta no espera una vinculación".into()) };
            let me = complete_link(p, dir, &Plain, answer)?;
            Store::open(dir, me.db_key())?;
            println!("dispositivo vinculado a la cuenta {}", me.id());
        }
        ["invitacion", addr] => {
            let (me, store) = open()?;
            let route = store.relay_route()?.map_or(addr.to_string(), |r| format!("{addr}|{r}"));
            println!("{}", invite(&me, &route));
        }
        ["usar-rele", relay] => {
            mobilemesh_core::transport::check_route(relay)?;
            let store = open()?.1;
            store.set_setting("rele", Some(relay))?;
            println!("relé {relay}; mi ruta por él: {}", store.relay_route()?.unwrap_or_default());
        }
        ["contacto", name, key, addr @ ..] if addr.len() <= 1 => {
            let (key, addr) = match parse_invite(key) {
                Ok((k, a)) => (k, Some(a)),
                Err(_) => (key.to_string(), addr.first().map(|a| a.to_string())),
            };
            open()?.1.add_contact(name, &key, addr.as_deref())?;
            println!("contacto añadido: {name}");
        }
        ["ayudante", addr, program] => {
            let (me, store) = open()?;
            store.set_setting("abierto", Some("1"))?;
            drop(store);
            let l = TcpListener::bind(addr)?;
            println!("ayudante escuchando en {} con «{program}»", l.local_addr()?);
            let (d, m) = (dir.to_path_buf(), me.clone());
            std::thread::spawn(move || {
                mobilemesh_core::accept_loop(l, d, m, || false, |r| {
                    if let Err(e) = r {
                        eprintln!("conexión rechazada: {e}");
                    }
                })
            });
            let mut last_retry = std::time::Instant::now();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(300));
                let store = Store::open(dir, me.db_key())?;
                let n = mobilemesh_core::funciones::serve_calls(&store, &me, |from, function, data| run_helper(program, from, function, data))?;
                if n > 0 {
                    println!("{n} llamada(s) atendida(s)");
                }
                if last_retry.elapsed().as_secs() >= 15 {
                    last_retry = std::time::Instant::now();
                    mobilemesh_core::funciones::retry_pending(&store, &me)?;
                }
            }
        }
        ["llamar", name, addr, function, data] => {
            let (me, store) = open()?;
            let helper = store.contact_key(name)?.ok_or("contacto desconocido")?;
            // Mi ruta para la respuesta (en el teléfono la pone la librería).
            let mine = std::env::var("MI_RUTA").map_err(|_| "falta MI_RUTA con mi dirección para la respuesta")?;
            let id = mobilemesh_core::funciones::call(&store, &me, &helper, addr, function, data, &mine)?;
            for _ in 0..100 {
                if let Some(r) = mobilemesh_core::funciones::reply(&store, &helper, &id)? {
                    match r {
                        Ok(v) => println!("{v}"),
                        Err(e) => return Err(format!("el ayudante respondió con error: {e}").into()),
                    }
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            return Err("el ayudante no respondió en 10 s".into());
        }
        ["escuchar", addr] => {
            let (me, store) = open()?;
            let l = TcpListener::bind(addr)?;
            println!("escuchando en {}", l.local_addr()?);
            let secret = store.mailbox_secret()?;
            for relay in store.relays()? {
                let (dir, me, secret) = (dir.to_path_buf(), me.clone(), secret.clone());
                println!("y en el relé {relay}");
                std::thread::spawn(move || {
                    mobilemesh_core::relay::listen(relay, secret, move |stream| {
                        let store = Store::open(&dir, me.db_key()).expect("base de datos");
                        match serve(&store, &me, stream) {
                            Ok(n) => println!("{n} mensaje(s) recibido(s) por el relé"),
                            Err(e) => eprintln!("conexión por el relé rechazada: {e}"),
                        }
                    })
                });
            }
            drop(store);
            mobilemesh_core::accept_loop(l, dir.to_path_buf(), me, || false, |r| match r {
                Ok(n) => println!("{n} mensaje(s) recibido(s) y guardado(s)"),
                Err(e) => eprintln!("conexión rechazada: {e}"),
            });
        }
        ["enviar", name, addr, text @ ..] | ["reintentar", name, addr, text @ ..] if text.len() <= 1 => {
            let (me, store) = open()?;
            let peer = store.contact_key(name)?.ok_or("contacto desconocido")?;
            match (cmd[0], text) {
                ("enviar", [text]) => drop(store.queue(&peer, text)?),
                ("reintentar", []) => {}
                _ => return Err(USO.into()),
            }
            match mobilemesh_core::send_or_deposit(&store, &me, &peer, addr) {
                Ok(mobilemesh_core::Sent::Delivered(n)) => println!("{n} mensaje(s) entregado(s): el destino confirmó que los guardó"),
                Ok(mobilemesh_core::Sent::Deposited(n)) => println!("{n} mensaje(s) dejado(s) en su buzón del relé: lo recogerá al despertar"),
                Err(e) => println!("no entregado ({e}); queda pendiente para reintentar"),
            }
        }
        ["buzon"] => {
            let (me, store) = open()?;
            println!("{} mensaje(s) nuevo(s) recogido(s) del buzón", mobilemesh_core::collect_letters(&store, &me)?);
        }
        ["mensajes"] => {
            for r in open()?.1.list()? {
                let dir = if r.outgoing { "para" } else { "de" };
                match &r.file {
                    None => println!("{dir:>4} {:<10} [{:<9}] {}", r.peer, r.status, r.body),
                    Some(f) => println!(
                        "{dir:>4} {:<10} [{:<9}] archivo «{}» {} bytes, {}% (id {})",
                        r.peer, r.status, f.name, f.size, r.progress, &r.id[..8]
                    ),
                }
            }
        }
        ["archivo", name, addr, path] => {
            let (me, store) = open()?;
            let peer = store.contact_key(name)?.ok_or("contacto desconocido")?;
            let path = Path::new(path);
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("archivo");
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let mime = match ext.as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "pdf" => "application/pdf",
                "mp4" => "video/mp4",
                "txt" => "text/plain",
                _ => "application/octet-stream",
            };
            mobilemesh_core::queue_file(&store, &peer, path, file_name, mime)?;
            match deliver(&store, &me, &peer, addr) {
                Ok(n) => println!("{n} mensaje(s) entregado(s): el destino confirmó que los guardó"),
                Err(e) => println!("no entregado ({e}); queda pendiente para reintentar"),
            }
        }
        ["aceptar-copias", name] => {
            let store = open()?.1;
            let key = store.contact_key(name)?.ok_or("contacto desconocido")?;
            store.set_consent(&key, true)?;
            println!("aceptas guardar copias cifradas de {name} (no podrás leerlas)");
        }
        ["copias"] => {
            for (owner, name, size) in open()?.1.held_list()? {
                println!("guardo copia de {} ({size} bytes)", name.unwrap_or(owner));
            }
        }
        ["sincronizar", route] => {
            let (me, store) = open()?;
            let (got, sent) = mobilemesh_core::sync_with(&store, &me, route)?;
            println!("{got} cambio(s) recibido(s), {sent} enviado(s)");
        }
        ["compartir", collection, name, access] => {
            let store = open()?.1;
            let key = store.contact_key(name)?.ok_or("contacto desconocido")?;
            let access = match *access {
                "leer" => Some(false),
                "escribir" => Some(true),
                "nada" => None,
                _ => return Err("usa: leer | escribir | nada".into()),
            };
            store.set_share(collection, &key, access)?;
            println!("hecho");
        }
        ["sincronizar-con", name, route] => {
            let (me, store) = open()?;
            let key = store.contact_key(name)?.ok_or("contacto desconocido")?;
            let (got, sent, rejected) = mobilemesh_core::sync_shared(&store, &me, route, &key)?;
            println!("{got} recibido(s), {sent} enviado(s), {rejected} rechazado(s)");
        }
        ["dato", collection, record, field, value] => {
            let (me, store) = open()?;
            let device = mobilemesh_core::identity::to_hex(&me.public);
            mobilemesh_core::sync::put_as(&store, &device, &me.id(), mobilemesh_core::sync::Policy::Lww, collection, record, field, Some(value))?;
            println!("guardado");
        }
        ["datos", collection] => {
            for r in mobilemesh_core::sync::records(&open()?.1, collection)? {
                println!("{}: {:?}", r.id, r.fields);
            }
        }
        ["exportar", id, dst] => {
            let store = open()?.1;
            let r = store.list()?.into_iter().find(|r| r.file.is_some() && r.id.starts_with(id)).ok_or("no hay ningún archivo con ese id")?;
            mobilemesh_core::export_file(&store, &r.peer_key, r.outgoing, &r.id, Path::new(dst))?;
            println!("descifrado y comprobado en {dst}");
        }
        ["copia"] => println!("{}", recovery::export(&Identity::load(dir, &Plain)?, &password()?)?),
        ["router"] => {
            let (control, local) = mobilemesh_core::upnp::router()?;
            println!("router con UPnP: {control} (mi IP local: {local})");
        }
        ["directorio"] => {
            let store = open()?.1;
            let nuevo = mobilemesh_core::directorio::actualizar(&store)?;
            println!("{}", if nuevo { "directorio nuevo guardado" } else { "sin cambios" });
            println!("relés: {:?}", mobilemesh_core::directorio::reles(&store));
            println!("guardianes: {:?}", mobilemesh_core::directorio::guardianes(&store));
        }
        ["guardian", addr, public] => {
            let (me, store) = open()?;
            mobilemesh_core::guardian::enable(&store)?;
            drop(store);
            let l = TcpListener::bind(addr)?;
            println!("guardián escuchando en {}", l.local_addr()?);
            println!("invitación: {}", invite(&me, public));
            // Cada 6 horas, borrar las copias abandonadas.
            let (d, m) = (dir.to_path_buf(), me.clone());
            std::thread::spawn(move || loop {
                if let Ok(n @ 1..) = Store::open(&d, m.db_key()).and_then(|s| mobilemesh_core::guardian::purge(&s)) {
                    println!("{n} copia(s) abandonada(s) borrada(s)");
                }
                std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
            });
            mobilemesh_core::accept_loop(l, dir.to_path_buf(), me, || false, |r| {
                if let Err(e) = r {
                    eprintln!("conexión rechazada: {e}");
                }
            });
        }
        ["nombres", addr, public] => {
            let (me, store) = open()?;
            mobilemesh_core::nombres::enable(&store)?;
            drop(store);
            let l = TcpListener::bind(addr)?;
            println!("guía de nombres escuchando en {}", l.local_addr()?);
            println!("invitación: {}", invite(&me, public));
            mobilemesh_core::accept_loop(l, dir.to_path_buf(), me, || false, |r| {
                if let Err(e) = r {
                    eprintln!("conexión rechazada: {e}");
                }
            });
        }
        ["registrar-nombre", guia, nombre, ruta] => {
            let (me, store) = open()?;
            mobilemesh_core::nombres::register(&store, &me, guia, nombre, &invite(&me, ruta))?;
            println!("nombre registrado: {nombre}");
        }
        ["buscar-nombre", guia, nombre] => {
            let (me, store) = open()?;
            println!("{}", mobilemesh_core::nombres::find(&store, &me, guia, nombre)?.unwrap_or_else(|| "no existe".into()));
        }
        ["pedir-contacto", guia, nombre] => {
            let (me, store) = open()?;
            let inv = mobilemesh_core::nombres::find(&store, &me, guia, nombre)?.ok_or("ese nombre no existe")?;
            let (key, ruta) = parse_invite(&inv)?;
            store.add_contact(nombre, &key, Some(&ruta))?;
            mobilemesh_core::nombres::request(&store, &me, guia, nombre)?;
            println!("contacto añadido y avisado: {nombre}");
        }
        ["solicitudes", guia] => {
            let (me, store) = open()?;
            for (nombre, inv) in mobilemesh_core::nombres::requests(&store, &me, guia)? {
                let (key, ruta) = parse_invite(&inv)?;
                store.add_contact(&nombre, &key, Some(&ruta))?;
                println!("contacto añadido por solicitud: {nombre}");
            }
        }
        ["guardar-cuenta", inv, user] => {
            let (me, store) = open()?;
            let (key, route) = parse_invite(inv)?;
            mobilemesh_core::guardian::save_account(&store, &me, &route, &key, user, &password()?)?;
            println!("código de recuperación guardado en el guardián");
        }
        ["buscar-cuenta", inv, user] => {
            if Identity::load(dir, &Plain).is_ok() {
                return Err("esta carpeta ya tiene una identidad válida; no se reemplaza".into());
            }
            let (key, route) = parse_invite(inv)?;
            let me = mobilemesh_core::guardian::find_account(&dir.with_extension("paso"), &route, &key, user, &password()?)?;
            me.save(dir, &Plain)?;
            Store::open(dir, me.db_key())?;
            println!("cuenta recuperada: {}", me.id());
        }
        ["restaurar", code] => {
            if Identity::load(dir, &Plain).is_ok() {
                return Err("esta carpeta ya tiene una identidad válida; no se reemplaza".into());
            }
            let me = recovery::import(code, &password()?)?;
            me.save(dir, &Plain)?;
            Store::open(dir, me.db_key())?;
            println!("identidad recuperada: {}", me.id());
        }
        _ => return Err(USO.into()),
    }
    Ok(())
}

/// Ejecuta una función del ayudante: `<programa> <función>` con `{"de", "datos"}` por la entrada.
/// Como mucho 30 s; la salida es el resultado y, si falla, el error es lo que escriba por la salida
/// de errores.
fn run_helper(program: &str, from: &str, function: &str, data: &serde_json::Value) -> std::result::Result<String, String> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    if function.is_empty() || !function.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err("nombre de función no válido".into());
    }
    let mut child = Command::new(program)
        .arg(function)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar el programa del ayudante: {e}"))?;
    let input = serde_json::json!({ "de": from, "datos": data }).to_string();
    let _ = child.stdin.take().map(|mut i| i.write_all(input.as_bytes()));
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        if start.elapsed().as_secs() >= 30 {
            let _ = child.kill();
            return Err("la función tardó más de 30 s".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let (mut out, mut err) = (String::new(), String::new());
    let _ = child.stdout.take().map(|mut o| o.read_to_string(&mut out));
    let _ = child.stderr.take().map(|mut e| e.read_to_string(&mut err));
    if status.success() { Ok(out.trim().to_string()) } else { Err(err.trim().to_string()) }
}
