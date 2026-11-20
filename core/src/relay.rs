//! Relé (Fase 9): une dos conexiones cuando los dispositivos no pueden conectarse directamente
//! (redes distintas, datos móviles, routers). Cualquiera puede ejecutar uno (`mesh-rele`).
//!
//! - Cada dispositivo mantiene una conexión **saliente** con el relé registrando un **buzón**: un
//!   identificador que no tiene que ver con su cuenta. El buzón es la huella de una clave pública;
//!   para registrarlo hay que firmar un número al azar que manda el relé, así que **el secreto nunca
//!   viaja** y quien espíe la red no puede quedarse con el buzón (auditoría, Fase 13).
//! - Quien quiere llamar pide al relé ese buzón; el relé avisa al dueño, que abre otra conexión, y
//!   el relé **empalma** las dos. Por ellas va el saludo Noise de extremo a extremo: el relé solo
//!   ve bytes cifrados.
//! - El relé no guarda nada ni escribe registros. Sí ve: direcciones IP, horas, tamaños y qué
//!   buzón habla con qué conexión.
//! - Límites contra la saturación: conexiones en total y por IP, buzones por IP y llamadas en espera
//!   por buzón; la conexión de control se cierra si deja de dar señales.
//!
//! - **Buzón de cartas** (para quien está dormido): cualquiera puede dejar una carta cifrada de
//!   extremo a extremo en un buzón; solo el dueño (con la misma firma) puede recogerlas. El relé no
//!   puede leerlas, las guarda en memoria como mucho 7 días y las borra al confirmarse.
//!
//! Protocolo de control: líneas de texto al empezar cada conexión.
//!   REGISTRAR <clave pública> → RETO <número> → FIRMA <firma> → OK <buzón>
//!                               (y después: LLAMADA <ficha> por cada llamada)
//!   MARCAR <buzón>            → OK | NO      (con OK, la conexión queda empalmada)
//!   ACEPTAR <ficha>           → (la conexión queda empalmada con quien marcó)
//!   DEPOSITAR <buzón> <carta> → OK | NO
//!   RECOGER <clave pública>   → RETO → FIRMA → CARTA <ficha> <carta>… FIN; el dueño contesta
//!                               RECIBIDA <ficha> por cada una y la borra el relé
//!   AVISO <clave pública>     → RETO → FIRMA → <ficha de avisos o «-» para quitarla> → OK
//!   DIRECTORIO                → <directorio firmado> | NO
//!   QUIEN                     → <la IP con la que el relé me ve>
//!   SUPERNODO <puerto> [wifi|-] [b=batería] [c=0|1 enchufado] [u=buzones] → OK | NO
//!   SUPERNODOS                → lista separada por comas de «pública» o «pública>wifi»
//!   SUPERNODOS+               → la misma lista con «;b=NN;c=0|1;u=NN» detrás de cada uno, para
//!                               elegir por calidad; un relé antiguo no la conoce
//!
//! **Avisos (opcional):** si el relé se arranca con un programa de avisos (p. ej. el que avisa por
//! Firebase), al llegar una carta a un buzón con ficha de avisos lo ejecuta con la ficha como
//! argumento. El aviso no lleva contenido: solo despierta al teléfono para que recoja su buzón.

/// Relés que usa toda app que no elija los suyos: así funciona entre redes distintas sin
/// configurar nada. Se prueban en orden y el buzón se registra en todos.
// ponytail: lista fija en el programa; un directorio de relés que se actualice solo si crece mucho.
// El primero es el nubia de casa (app «Relé MobileMesh»), por su nombre fijo de DuckDNS.
pub const UNIVERSALES: &[&str] = &["mobilemeshjuan.duckdns.org:7900"];

/// Los 2 relés de `all` que usa el buzón con este secreto: cada teléfono usa solo 2, así la carga se
/// reparte entre todos los de la lista.
pub fn choose(all: &[&str], secret: &str) -> Vec<String> {
    let h = Sha256::digest(secret.as_bytes());
    let start = (u64::from_le_bytes(h[..8].try_into().unwrap_or_default()) % all.len().max(1) as u64) as usize;
    (0..all.len().min(2)).map(|i| all[(start + i) % all.len()].to_string()).collect()
}

/// Valor del ajuste «rele» que significa «ningún relé, ni los universales».
pub const SIN_RELE: &str = "ninguno";

// El relé en sí (programa de servidor, licencia AGPL) está en el proyecto aparte `servidor/`.

use crate::{
    Result,
    net::Conn,
    identity::{from_hex_any, to_hex},
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::IpAddr,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[doc(hidden)]
pub const WAIT: Duration = Duration::from_secs(10);
#[doc(hidden)]
pub const CONTEXT: &[u8] = b"mobilemesh-buzon-v1";
/// Cartas: tamaño máximo, cuántas por buzón, total en memoria y cuánto se guardan.
pub const MAX_LETTER: usize = 48 * 1024;

/// Lee una línea sin leer de más (lo que venga detrás es tráfico Noise del otro lado).
pub fn read_line(s: &mut impl Read) -> Result<String> {
    read_line_max(s, 200)
}

#[doc(hidden)]
pub fn read_line_max(s: &mut impl Read, max: usize) -> Result<String> {
    let mut line = Vec::new();
    let mut b = [0u8; 1];
    while line.len() < max {
        if s.read(&mut b)? == 0 {
            return Err("el relé cerró la conexión".into());
        }
        if b[0] == b'\n' {
            return Ok(String::from_utf8(line)?);
        }
        line.push(b[0]);
    }
    Err("línea demasiado larga".into())
}

/// Clave de firma de un buzón, sacada de su secreto.
fn mailbox_key(secret: &str, app: &str) -> SigningKey {
    // El App ID entra en la derivación: el mismo secreto da buzones distintos en cada app.
    SigningKey::from_bytes(
        &Sha256::new_with_prefix(CONTEXT)
            .chain_update(app.as_bytes())
            .chain_update(b"\0")
            .chain_update(secret.as_bytes())
            .finalize()
            .into(),
    )
}

#[doc(hidden)]
pub fn id_of(public: &[u8]) -> String {
    to_hex(&Sha256::digest(public)[..16])
}

/// Identificador público de un buzón a partir de su secreto y su app.
pub fn mailbox_id(secret: &str, app: &str) -> String {
    id_of(&mailbox_key(secret, app).verifying_key().to_bytes())
}

/// Mi IP pública tal como la ve el relé.
pub fn public_ip(relay: &str) -> Result<String> {
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "QUIEN")?;
    let ip: IpAddr = read_line(&mut s)?.parse().map_err(|_| "el relé no dio una IP")?;
    Ok(ip.to_string())
}

/// Pide al relé el directorio firmado que sirve (la firma se comprueba fuera).
pub fn directory(relay: &str) -> Result<String> {
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "DIRECTORIO")?;
    match read_line_max(&mut s, 8192)?.as_str() {
        "NO" => Err("el relé no tiene directorio".into()),
        l => Ok(l.to_string()),
    }
}

/// Llama al buzón `id` a través del relé. Devuelve la conexión ya empalmada con su dueño.
pub fn dial(relay: &str, id: &str) -> Result<crate::net::Stream> {
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT + Duration::from_secs(2)))?;
    writeln!(s, "MARCAR {id}")?;
    match read_line(&mut s)?.as_str() {
        "OK" => Ok(s),
        _ => Err("el destinatario no está conectado al relé o está ocupado; se reintentará".into()),
    }
}

/// Deja una carta (ya cifrada de extremo a extremo, en base64) en el buzón `id`.
pub fn deposit(relay: &str, id: &str, letter: &str) -> Result<()> {
    if letter.len() > MAX_LETTER {
        return Err("carta demasiado grande".into());
    }
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "DEPOSITAR {id} {letter}")?;
    match read_line(&mut s)?.as_str() {
        "OK" => Ok(()),
        _ => Err("el relé no aceptó la carta (buzón lleno)".into()),
    }
}

/// Guarda en el relé la ficha de avisos (p. ej. de Firebase) del buzón de `secret`, o la quita con `None`.
pub fn set_push_token(relay: &str, secret: &str, app: &str, token: Option<&str>) -> Result<()> {
    let key = mailbox_key(secret, app);
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "AVISO {}", to_hex(&key.verifying_key().to_bytes()))?;
    let challenge = read_line(&mut s)?;
    let nonce = from_hex_any(challenge.strip_prefix("RETO ").ok_or("el relé no mandó el reto")?)?;
    writeln!(s, "FIRMA {}", to_hex(&key.sign(&[CONTEXT, &nonce].concat()).to_bytes()))?;
    writeln!(s, "{}", token.unwrap_or("-"))?;
    match read_line(&mut s)?.as_str() {
        "OK" => Ok(()),
        _ => Err("el relé no aceptó la ficha de avisos".into()),
    }
}

/// Recoge las cartas del buzón de `secret`. `handle` recibe cada una y devuelve si se puede borrar.
/// Devuelve cuántas había.
pub fn collect(relay: &str, secret: &str, app: &str, mut handle: impl FnMut(&str) -> bool) -> Result<usize> {
    let key = mailbox_key(secret, app);
    let mut s = crate::net::connect(relay, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "RECOGER {}", to_hex(&key.verifying_key().to_bytes()))?;
    let challenge = read_line(&mut s)?;
    let nonce = from_hex_any(challenge.strip_prefix("RETO ").ok_or("el relé no mandó el reto")?)?;
    writeln!(s, "FIRMA {}", to_hex(&key.sign(&[CONTEXT, &nonce].concat()).to_bytes()))?;
    let mut n = 0;
    loop {
        let line = read_line_max(&mut s, MAX_LETTER + 100)?;
        if line == "FIN" {
            break;
        }
        let Some((token, letter)) = line.strip_prefix("CARTA ").and_then(|r| r.split_once(' ')) else { break };
        n += 1;
        if handle(letter) {
            writeln!(s, "RECIBIDA {token}")?;
        }
    }
    let _ = writeln!(s, "ADIOS");
    Ok(n)
}

/// Registra el buzón de `secret` en el relé. Devuelve la conexión de control, por la que llegan
/// las líneas «LLAMADA <ficha>».
pub fn register(relay: &str, secret: &str, app: &str) -> Result<crate::net::Stream> {
    let key = mailbox_key(secret, app);
    let mut control = crate::net::connect(relay, WAIT)?;
    control.set_read_timeout(Some(WAIT))?;
    writeln!(control, "REGISTRAR {}", to_hex(&key.verifying_key().to_bytes()))?;
    let challenge = read_line(&mut control)?;
    let nonce = from_hex_any(challenge.strip_prefix("RETO ").ok_or("el relé no mandó el reto")?)?;
    writeln!(control, "FIRMA {}", to_hex(&key.sign(&[CONTEXT, &nonce].concat()).to_bytes()))?;
    if !read_line(&mut control)?.starts_with("OK ") {
        return Err("el relé no aceptó el buzón".into());
    }
    Ok(control)
}

/// Contesta la llamada `token`: devuelve la conexión ya empalmada con quien llama.
pub fn answer(relay: &str, token: &str) -> Result<crate::net::Stream> {
    let mut s = crate::net::connect(relay, WAIT)?;
    writeln!(s, "ACEPTAR {token}")?;
    Ok(s)
}

/// Un supernodo como lo da el director, «pública» o «pública>wifi» → (pública, wifi).
pub fn supernode_parts(entrada: &str) -> (&str, Option<&str>) {
    match entrada.split_once('>') {
        Some((p, w)) => (p, Some(w)),
        None => (entrada, None),
    }
}

/// Supernodos vivos que da el director (el relé del proyecto): «ip:puerto».
pub fn supernodes(director: &str) -> Result<Vec<String>> {
    let mut s = crate::net::connect(director, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "SUPERNODOS")?;
    Ok(read_line_max(&mut s, 8192)?.split(',').map(str::trim).filter(|a| !a.is_empty()).map(String::from).collect())
}

/// La dirección por la que se le llega a un supernodo: la de su wifi si estamos en esa misma casa
/// (el router no deja llegar a la IP pública desde dentro) y, si no, la pública.
pub fn supernode_addr(entrada: &str) -> String {
    let (publica, wifi) = supernode_parts(entrada);
    wifi.filter(|w| w.parse().is_ok_and(|a| crate::transport::reachable(&a))).unwrap_or(publica).to_string()
}

// --- Elección por calidad (selección adaptativa de nodos de apoyo) ---------------------------
//
// Antes se elegía por sorteo: un hash del secreto del buzón repartía los clientes entre los
// supernodos vivos (`choose`). Reparte bien la carga, pero ignora por completo si el supernodo
// está lejos, saturado o a punto de quedarse sin batería. Aquí se elige por una puntuación de
// enlace, batería y carga, y se conserva el sorteo para desempatar. `choose` sigue intacta: es el
// camino de respaldo cuando el director es antiguo y no sabe dar métricas.

/// Un supernodo con lo que anuncia de sí mismo, como lo da el director en `SUPERNODOS+`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupernodeInfo {
    /// Dirección tal cual: «pública» o «pública>wifi».
    pub addr: String,
    /// Batería que le queda, en múltiplos de 10 % (0 si no la dice).
    pub bat: u8,
    /// Si está enchufado.
    pub cargando: bool,
    /// Buzones que atiende ahora mismo.
    pub uso: u16,
}

/// Buzones que admite un supernodo (`supernodo::MAX_CONEXIONES`).
const TOPE_BUZONES: f32 = 64.0;
/// A partir de este tiempo de ida y vuelta el enlace puntúa 0.
const RTT_TOPE: f32 = 1000.0;
/// Pesos de la puntuación: enlace, batería y carga. Se calibran con la prueba piloto.
const W_RTT: f32 = 0.5;
const W_BAT: f32 = 0.3;
const W_USO: f32 = 0.2;
/// Cuántos supernodos se miden de verdad: medir a todos gastaría datos y batería.
const CANDIDATOS: usize = 5;
/// Cuánto tiene que ganar el mejor para que compense cambiarse (histéresis).
pub const MEJORA_MINIMA: f32 = 1.20;
/// Por debajo de esta diferencia dos supernodos se consideran iguales y desempata el sorteo.
const EMPATE: f32 = 0.05;

/// Supernodos vivos **con sus métricas**. Un director antiguo no conoce `SUPERNODOS+`: entonces
/// esto falla y quien llame debe seguir con [`supernodes`] y [`choose`].
pub fn supernodes_info(director: &str) -> Result<Vec<SupernodeInfo>> {
    let mut s = crate::net::connect(director, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    writeln!(s, "SUPERNODOS+")?;
    let linea = read_line_max(&mut s, 16384)?;
    if linea.trim() == "NO" {
        return Err("el director no da métricas de supernodos".into());
    }
    Ok(linea.split(',').filter_map(parse_info).collect())
}

/// «pública[>wifi];b=80;c=1;u=3» → ficha. Al que no dice sus métricas se le supone lo peor, para
/// no premiar al que las calla.
pub fn parse_info(entrada: &str) -> Option<SupernodeInfo> {
    let mut trozos = entrada.trim().split(';');
    let addr = trozos.next()?.trim();
    if addr.is_empty() {
        return None;
    }
    let (mut bat, mut cargando, mut uso) = (0u8, false, TOPE_BUZONES as u16);
    for t in trozos {
        match t.trim().split_once('=') {
            Some(("b", v)) => bat = v.parse().unwrap_or(0).min(100),
            Some(("c", v)) => cargando = v == "1",
            Some(("u", v)) => uso = v.parse().unwrap_or(TOPE_BUZONES as u16).min(TOPE_BUZONES as u16),
            _ => {}
        }
    }
    Some(SupernodeInfo { addr: addr.to_string(), bat, cargando, uso })
}

/// Puntuación de 0 a 1: medio enlace, un tercio batería y el resto carga. `rtt` en milisegundos;
/// `None` es «no contestó», que vale 0: de nada sirve el supernodo al que no se llega.
pub fn score(sn: &SupernodeInfo, rtt: Option<u32>) -> f32 {
    let Some(rtt) = rtt else { return 0.0 };
    let q_rtt = 1.0 - (rtt as f32 / RTT_TOPE).min(1.0);
    let q_bat = (sn.bat as f32 / 100.0 + if sn.cargando { 0.2 } else { 0.0 }).min(1.0);
    let q_uso = 1.0 - (sn.uso as f32 / TOPE_BUZONES).min(1.0);
    W_RTT * q_rtt + W_BAT * q_bat + W_USO * q_uso
}

/// Tiempo de ida y vuelta hasta el supernodo, en milisegundos: se le pregunta `QUIEN`, lo mismo que
/// hace el director para comprobar que está vivo. El mejor de dos intentos; `None` si no contesta.
pub fn probe_rtt(addr: &str) -> Option<u32> {
    let plazo = Duration::from_secs(1);
    let medido = (0..2)
        .filter_map(|_| {
            let t = Instant::now();
            let mut s = crate::net::connect(addr, plazo).ok()?;
            s.set_read_timeout(Some(plazo)).ok()?;
            writeln!(s, "QUIEN").ok()?;
            read_line(&mut s).ok()?;
            u32::try_from(t.elapsed().as_millis()).ok()
        })
        .min();
    // La serie de tiempos a cada candidato es lo que sostiene la comparación del mecanismo
    // adaptativo con el sorteo anterior; sin ella solo queda la decisión, no el porqué.
    crate::metricas::registrar(crate::metricas::Fila {
        evento: "rtt",
        resultado: if medido.is_some() { "contesta" } else { "callado" },
        objetivo: addr,
        rtt_ms: medido,
        ..Default::default()
    });
    medido
}

/// Dónde empieza el reparto de este buzón: el mismo sorteo que usa [`choose`].
fn rotacion(n: usize, secret: &str) -> usize {
    let h = Sha256::digest(secret.as_bytes());
    (u64::from_le_bytes(h[..8].try_into().unwrap_or_default()) % n.max(1) as u64) as usize
}

/// El mejor supernodo y su puntuación, con los tiempos de ida y vuelta ya medidos (`rtts` va en el
/// mismo orden que `infos`). Entre los que empatan decide el sorteo de siempre, así dos teléfonos
/// con candidatos igual de buenos no se amontonan en el mismo.
pub fn mejor_con(infos: &[SupernodeInfo], rtts: &[Option<u32>], secret: &str) -> Option<(String, f32)> {
    if infos.is_empty() {
        return None;
    }
    let puntos: Vec<f32> = infos.iter().enumerate().map(|(i, sn)| score(sn, rtts.get(i).copied().flatten())).collect();
    let techo = puntos.iter().copied().fold(f32::MIN, f32::max);
    if techo <= 0.0 {
        return None; // ninguno contestó
    }
    let inicio = rotacion(infos.len(), secret);
    let gana = (0..infos.len()).map(|i| (inicio + i) % infos.len()).find(|&i| techo - puntos[i] < EMPATE)?;
    Some((infos[gana].addr.clone(), puntos[gana]))
}

/// Como [`mejor_con`], midiendo el enlace a los [`CANDIDATOS`] que mejor pinta tienen por batería y
/// carga. A los demás no se les mide: puntúan 0 y solo entran si ninguno de los medidos contesta.
pub fn mejor(infos: &[SupernodeInfo], secret: &str) -> Option<(String, f32)> {
    let mut orden: Vec<usize> = (0..infos.len()).collect();
    orden.sort_by(|&a, &b| score(&infos[b], Some(0)).total_cmp(&score(&infos[a], Some(0))));
    let miden: Vec<usize> = orden.into_iter().take(CANDIDATOS).collect();
    let rtts: Vec<Option<u32>> = (0..infos.len()).map(|i| miden.contains(&i).then(|| probe_rtt(&supernode_addr(&infos[i].addr))).flatten()).collect();
    mejor_con(infos, &rtts, secret)
}

/// Con qué supernodo quedarse: el actual, salvo que otro gane claro (más de un 20 %) **dos veces
/// seguidas**. Sin esa doble condición, dos supernodos parecidos se turnarían cada diez minutos, y
/// cada cambio cuesta soltar el buzón y registrarlo en el otro. `aspirante` lleva la cuenta de una
/// vuelta a la siguiente: quién va ganando y desde cuántas vueltas.
///
/// No toca la red (los puntos vienen ya medidos), así que se puede probar tal cual.
pub fn decidir_supernodo(actual: Option<(&str, f32)>, mejor: (&str, f32), aspirante: &mut Option<(String, u8)>) -> String {
    let (candidato, puntos) = mejor;
    let Some((actual, suyos)) = actual else {
        *aspirante = None;
        return candidato.to_string(); // no había ninguno: se coge el mejor y ya
    };
    if candidato == actual || !cambiar(suyos, puntos) {
        *aspirante = None;
        return actual.to_string();
    }
    match aspirante {
        Some((quien, veces)) if quien == candidato => {
            *veces += 1;
            if *veces >= 2 {
                *aspirante = None;
                return candidato.to_string();
            }
        }
        _ => *aspirante = Some((candidato.to_string(), 1)),
    }
    actual.to_string()
}

/// ¿Compensa dejar el supernodo actual por el mejor? Solo si gana por más de [`MEJORA_MINIMA`]:
/// cambiar cuesta soltar el buzón y registrarlo en otro, y sin este freno dos supernodos parecidos
/// se turnarían sin parar.
pub fn cambiar(actual: f32, mejor: f32) -> bool {
    actual <= 0.0 || mejor > actual * MEJORA_MINIMA
}

/// Mantiene el buzón registrado en el relé y entrega cada llamada a `on_call`.
/// Si se cae, vuelve a conectar (esperando cada vez más, hasta 1 minuto). Nunca termina.
pub fn listen(relay: String, secret: String, app: String, on_call: impl Fn(crate::net::Stream) + Send + Sync + 'static) {
    escuchar(relay, secret, app, || true, None, on_call)
}

/// Como `listen`, pero termina cuando `seguir` deja de cumplirse (se mira al menos cada 2 min).
/// Para supernodos: cuando se elige otro, el buzón en el viejo se suelta.
pub fn listen_while(relay: String, secret: String, app: String, seguir: impl Fn() -> bool, on_call: impl Fn(crate::net::Stream) + Send + Sync + 'static) {
    escuchar(relay, secret, app, seguir, Some(Duration::from_secs(120)), on_call)
}

fn escuchar(relay: String, secret: String, app: String, seguir: impl Fn() -> bool, cada: Option<Duration>, on_call: impl Fn(crate::net::Stream) + Send + Sync + 'static) {
    let on_call = Arc::new(on_call);
    let mut pause = Duration::from_secs(2);
    while seguir() {
        let session = (|| -> Result<()> {
            let mut control = register(&relay, &secret, &app)?;
            pause = Duration::from_secs(2);
            // Un «PING» de vez en cuando mantiene abierta la conexión en routers y redes móviles.
            let mut ping = control.try_clone_conn()?;
            thread::spawn(move || {
                while writeln!(ping, "PING").is_ok() {
                    thread::sleep(Duration::from_secs(25));
                }
            });
            // Con `cada`, la lectura despierta de vez en cuando para mirar si hay que seguir.
            control.set_read_timeout(cada)?;
            loop {
                let line = read_line(&mut control)?;
                let Some(token) = line.strip_prefix("LLAMADA ") else { continue };
                let (relay, token, on_call) = (relay.clone(), token.to_string(), on_call.clone());
                thread::spawn(move || {
                    if let Ok(s) = answer(&relay, &token) {
                        on_call(s);
                    }
                });
            }
        })();
        let _ = session;
        if !seguir() {
            return;
        }
        thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_secs(60));
    }
}
