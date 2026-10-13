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
        ["escuchar", addr] => {
            let (me, store) = open()?;
            let l = TcpListener::bind(addr)?;
            println!("escuchando en {}", l.local_addr()?);
            if let Some((relay, secret)) = store.relay()? {
                let (dir, me) = (dir.to_path_buf(), me.clone());
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
