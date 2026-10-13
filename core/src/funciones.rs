//! Funciones de servidor sin servidor central: un **ayudante** es un MobileMesh sin pantalla que
//! corre donde quiera el programador (su servidor, una Raspberry…). La app le llama con un mensaje
//! cifrado de extremo a extremo; el ayudante ejecuta un programa del programador y responde igual.
//!
//! - El ayudante sabe con certeza qué cuenta le llama (la del saludo Noise, firmada).
//! - La respuesta usa el mismo camino que cualquier mensaje: directo, relé o buzón si el teléfono
//!   duerme.
//!
//! Las llamadas y respuestas viajan como mensajes con un prefijo invisible y no se muestran en la
//! lista de mensajes.

use crate::{Identity, Result, Store, send_or_deposit};
use serde::{Deserialize, Serialize};

/// Prefijo de los mensajes de control (llamadas y respuestas); empieza por un carácter invisible.
pub const CONTROL: char = '\u{1}';
const CALL: &str = "\u{1}mm-llamada:";
const REPLY: &str = "\u{1}mm-respuesta:";
/// Tamaño máximo de los datos de una llamada o de una respuesta.
pub const MAX_DATA: usize = 32 * 1024;

#[derive(Serialize, Deserialize)]
struct Call {
    funcion: String,
    datos: serde_json::Value,
    /// Cómo devolver la respuesta a quien llama (su invitación sin la cuenta).
    ruta: String,
}

#[derive(Serialize, Deserialize)]
struct Reply {
    /// Identificador del mensaje de la llamada.
    llamada: String,
    ok: bool,
    resultado: serde_json::Value,
}

/// ¿Es un mensaje de control (que no debe mostrarse como mensaje normal)?
pub fn is_control(body: &str) -> bool {
    body.starts_with(CONTROL)
}

/// Llama a la función `funcion` del ayudante `helper` (cuenta) con `datos` (JSON). `my_route` es
/// mi ruta para la respuesta. Devuelve el identificador de la llamada (para esperar la respuesta).
pub fn call(store: &Store, me: &Identity, helper: &str, helper_route: &str, funcion: &str, datos: &str, my_route: &str) -> Result<String> {
    if datos.len() > MAX_DATA {
        return Err("los datos de la llamada son demasiado grandes".into());
    }
    let datos: serde_json::Value = serde_json::from_str(datos).map_err(|_| "los datos de la llamada no son JSON válido")?;
    let body = format!("{CALL}{}", serde_json::to_string(&Call { funcion: funcion.into(), datos, ruta: my_route.into() })?);
    let id = store.queue(helper, &body)?;
    send_or_deposit(store, me, helper, helper_route)?;
    Ok(id)
}

/// Respuesta a la llamada `call_id`, si ya llegó: `Ok(Some(Ok(resultado)))` o `Ok(Some(Err(error)))`.
pub fn reply(store: &Store, helper: &str, call_id: &str) -> Result<Option<std::result::Result<String, String>>> {
    for m in store.list()? {
        if m.outgoing || m.peer_key != helper {
            continue;
        }
        let Some(json) = m.body.strip_prefix(REPLY) else { continue };
        let Ok(r) = serde_json::from_str::<Reply>(json) else { continue };
        if r.llamada == call_id {
            let text = match r.resultado {
                serde_json::Value::String(s) => s,
                v => v.to_string(),
            };
            return Ok(Some(if r.ok { Ok(text) } else { Err(text) }));
        }
    }
    Ok(None)
}

/// Para el ayudante: atiende las llamadas recibidas que aún no se atendieron. `run(cuenta, función,
/// datos)` ejecuta la función y devuelve el resultado (JSON o texto) o un error. Devuelve cuántas.
// ponytail: recorre todos los mensajes en cada pasada; guardar un índice si el ayudante acumula muchísimos.
pub fn serve_calls(store: &Store, me: &Identity, run: impl Fn(&str, &str, &serde_json::Value) -> std::result::Result<String, String>) -> Result<usize> {
    let mut n = 0;
    for m in store.list()? {
        if m.outgoing || m.status == "atendida" {
            continue;
        }
        let Some(json) = m.body.strip_prefix(CALL) else { continue };
        store.set_status(&m.peer_key, false, &m.id, "atendida")?;
        let (ok, resultado, ruta) = match serde_json::from_str::<Call>(json) {
            Ok(c) => match run(&m.peer_key, &c.funcion, &c.datos) {
                Ok(r) => (true, serde_json::from_str(&r).unwrap_or(serde_json::Value::String(r)), c.ruta),
                Err(e) => (false, serde_json::Value::String(e), c.ruta),
            },
            Err(_) => continue, // mal formada: no hay ruta a la que contestar
        };
        let mut body = format!("{REPLY}{}", serde_json::to_string(&Reply { llamada: m.id.clone(), ok, resultado })?);
        if body.len() > MAX_DATA + 200 {
            body = format!("{REPLY}{}", serde_json::to_string(&Reply { llamada: m.id, ok: false, resultado: "la respuesta es demasiado grande".into() })?);
        }
        store.queue(&m.peer_key, &body)?;
        // Apuntar su ruta más reciente y responder. Si el teléfono no contesta, queda en su buzón o
        // pendiente, y `retry_pending` lo reintenta.
        if crate::transport::check_route(&ruta).is_ok() {
            if let Some(name) = store.contact_name(&m.peer_key)? {
                store.add_contact(&name, &m.peer_key, Some(&ruta))?;
            }
            let _ = send_or_deposit(store, me, &m.peer_key, &ruta);
        }
        n += 1;
    }
    Ok(n)
}

/// Para el ayudante: reintenta las respuestas que quedaron pendientes (teléfono sin conexión).
pub fn retry_pending(store: &Store, me: &Identity) -> Result<()> {
    for (_, key, addr) in store.contacts()? {
        if let Some(addr) = addr
            && !store.pending(&key)?.is_empty()
        {
            let _ = send_or_deposit(store, me, &key, &addr);
        }
    }
    Ok(())
}
