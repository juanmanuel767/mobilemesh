//! Puente WebSocket del relé (pieza aparte): los navegadores no pueden abrir conexiones TCP, solo
//! WebSocket. Este puente acepta WebSocket en otro puerto y pasa los bytes tal cual a una conexión
//! TCP con el relé de siempre. El relé no cambia y el contenido sigue cifrado de extremo a extremo.

use crate::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

/// Tamaño máximo de un mensaje WebSocket (las cartas del relé son de hasta 48 KB).
const MAX_FRAME: u64 = 1024 * 1024;
const MAX_POR_IP: usize = 1000;
const WAIT: Duration = Duration::from_secs(10);

/// Atiende WebSocket en `listener` y empalma cada conexión con el relé en `relay`. Con `tls`
/// (certificado y clave en PEM) acepta también `wss://` en el mismo puerto: si lo primero que llega
/// es un saludo TLS, se quita el candado y se vuelve a entrar por este mismo puerto en claro.
// ponytail: el relé ve las conexiones web como 127.0.0.1; el límite por IP real se aplica aquí.
pub fn serve(listener: TcpListener, relay: SocketAddr, tls: Option<(String, String)>) {
    let yo = listener.local_addr().map(|a| SocketAddr::from(([127, 0, 0, 1], a.port())));
    let por_ip: Arc<Mutex<HashMap<IpAddr, usize>>> = Arc::default();
    for stream in listener.incoming().flatten() {
        let Ok(ip) = stream.peer_addr().map(|a| a.ip()) else { continue };
        {
            let mut m = por_ip.lock().unwrap_or_else(|e| e.into_inner());
            let n = m.entry(ip).or_insert(0);
            // Las locales vienen del candado (wss), que ya contó la IP real.
            if *n >= MAX_POR_IP && !ip.is_loopback() {
                continue;
            }
            *n += 1;
        }
        let por_ip = por_ip.clone();
        let (tls, yo) = (tls.clone(), yo.as_ref().ok().copied());
        thread::spawn(move || {
            let mut primero = [0u8; 1];
            let es_tls = stream.peek(&mut primero).is_ok_and(|n| n == 1 && primero[0] == 0x16);
            #[cfg(not(target_arch = "wasm32"))]
            if let (true, Some((cert, key)), Some(yo)) = (es_tls, &tls, yo) {
                let _ = crate::tls::config(cert, key).and_then(|cfg| crate::tls::terminate(stream, cfg, yo));
            } else {
                let _ = bridge(stream, relay);
            }
            #[cfg(target_arch = "wasm32")]
            let _ = (es_tls, tls, yo, bridge(stream, relay));
            let mut m = por_ip.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(n) = m.get_mut(&ip) {
                *n -= 1;
                if *n == 0 {
                    m.remove(&ip);
                }
            }
        });
    }
}

fn bridge(mut web: TcpStream, relay: SocketAddr) -> Result<()> {
    web.set_read_timeout(Some(WAIT))?;
    handshake(&mut web)?;
    web.set_read_timeout(None)?;
    let tcp = TcpStream::connect_timeout(&relay, WAIT)?;
    let (mut web_r, mut tcp_w) = (web.try_clone()?, tcp.try_clone()?);
    let (mut tcp_r, mut web_w) = (tcp, web);
    // Navegador → relé: mensajes WebSocket → bytes.
    let subida = thread::spawn(move || {
        while let Ok(Some(datos)) = read_frame(&mut web_r) {
            if tcp_w.write_all(&datos).is_err() {
                break;
            }
        }
        let _ = tcp_w.shutdown(Shutdown::Both);
        let _ = web_r.shutdown(Shutdown::Both);
    });
    // Relé → navegador: bytes → mensajes WebSocket binarios.
    let mut buf = [0u8; 16 * 1024];
    while let Ok(n @ 1..) = tcp_r.read(&mut buf) {
        if write_frame(&mut web_w, &buf[..n]).is_err() {
            break;
        }
    }
    let _ = web_w.write_all(&[0x88, 0]); // cierre
    let _ = web_w.shutdown(Shutdown::Both);
    let _ = tcp_r.shutdown(Shutdown::Both);
    let _ = subida.join();
    Ok(())
}

/// Saludo HTTP de WebSocket (RFC 6455): responde 101 con la clave de aceptación.
fn handshake(s: &mut TcpStream) -> Result<()> {
    // Plazo TOTAL (no por byte): si no, se puede retener la conexión mandando un byte cada poco.
    let limite = std::time::Instant::now() + WAIT;
    let mut req = Vec::new();
    let mut b = [0u8; 1];
    while !req.ends_with(b"\r\n\r\n") {
        if std::time::Instant::now() > limite || req.len() > 8192 || s.read(&mut b)? == 0 {
            return Err("saludo WebSocket no válido".into());
        }
        req.push(b[0]);
    }
    let req = String::from_utf8_lossy(&req);
    let key = req
        .lines()
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("sec-websocket-key")).map(|(_, v)| v.trim().to_string()))
        .ok_or("falta Sec-WebSocket-Key")?;
    let accept = B64.encode(sha1(format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes()));
    write!(s, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n")?;
    Ok(())
}

/// Lee un mensaje del navegador (siempre enmascarado). `None` al cerrar.
fn read_frame(s: &mut TcpStream) -> Result<Option<Vec<u8>>> {
    loop {
        let mut h = [0u8; 2];
        s.read_exact(&mut h)?;
        let opcode = h[0] & 0x0f;
        let mut len = (h[1] & 0x7f) as u64;
        if len == 126 {
            let mut l = [0u8; 2];
            s.read_exact(&mut l)?;
            len = u16::from_be_bytes(l) as u64;
        } else if len == 127 {
            let mut l = [0u8; 8];
            s.read_exact(&mut l)?;
            len = u64::from_be_bytes(l);
        }
        if len > MAX_FRAME || h[1] & 0x80 == 0 {
            return Err("mensaje WebSocket no válido".into());
        }
        let mut mask = [0u8; 4];
        s.read_exact(&mut mask)?;
        let mut data = vec![0u8; len as usize];
        s.read_exact(&mut data)?;
        data.iter_mut().enumerate().for_each(|(i, b)| *b ^= mask[i % 4]);
        match opcode {
            0x8 => return Ok(None),
            0x9 => {
                // ping → pong con los mismos datos
                let mut pong = vec![0x8a];
                pong.extend(len_bytes(data.len()));
                pong.extend(&data);
                s.write_all(&pong)?;
            }
            0xa => {}
            _ => return Ok(Some(data)),
        }
    }
}

fn write_frame(s: &mut TcpStream, data: &[u8]) -> Result<()> {
    let mut f = vec![0x82];
    f.extend(len_bytes(data.len()));
    f.extend_from_slice(data);
    s.write_all(&f)?;
    Ok(())
}

fn len_bytes(n: usize) -> Vec<u8> {
    match n {
        0..=125 => vec![n as u8],
        126..=65535 => [vec![126], (n as u16).to_be_bytes().to_vec()].concat(),
        _ => [vec![127], (n as u64).to_be_bytes().to_vec()].concat(),
    }
}

/// SHA-1, solo para la clave del saludo WebSocket (lo exige el estándar; no protege nada).
fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut m = msg.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend(((msg.len() as u64) * 8).to_be_bytes());
    for chunk in m.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap_or_default());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, t);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn sha1_y_clave_de_aceptacion_del_estandar() {
        // Ejemplo del RFC 6455, sección 1.3.
        use base64::Engine as _;
        let k = super::sha1(b"dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
        assert_eq!(base64::engine::general_purpose::STANDARD.encode(k), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
