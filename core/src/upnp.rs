//! Abrir un puerto en el router de casa por UPnP (pieza aparte). Así un teléfono en una wifi con
//! UPnP recibe conexiones directas desde otras redes (también desde datos móviles) sin el relé.
//! Sin UPnP (o en datos móviles) simplemente no se abre nada y se sigue usando el relé.

use crate::Result;
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs, UdpSocket},
    time::Duration,
};

const SERVICIO: &str = "urn:schemas-upnp-org:service:WANIPConnection:1";
const WAIT: Duration = Duration::from_secs(4);

/// Pide al router que abra un puerto público hacia `port` de este equipo durante `seconds`.
/// Si `port` ya lo usa otro aparato de la casa, prueba los siguientes. Devuelve el puerto público.
pub fn open(port: u16, seconds: u32) -> Result<u16> {
    let (control, local) = control()?;
    for public in port..port.saturating_add(20) {
        let clave = format!("<NewRemoteHost></NewRemoteHost><NewExternalPort>{public}</NewExternalPort><NewProtocol>TCP</NewProtocol>");
        // Libre, o ya era mío (aunque el router lo tenga a nombre de otra IP mía no se sabe: se salta).
        match soap(&control, "GetSpecificPortMappingEntry", &clave) {
            Ok(r) if !r.contains(&format!("<NewInternalClient>{local}<")) => continue,
            Ok(_) => drop(soap(&control, "DeletePortMapping", &clave)),
            Err(_) => {}
        }
        soap(
            &control,
            "AddPortMapping",
            &format!(
                "{clave}<NewInternalPort>{port}</NewInternalPort><NewInternalClient>{local}</NewInternalClient>\
                 <NewEnabled>1</NewEnabled><NewPortMappingDescription>mobilemesh</NewPortMappingDescription>\
                 <NewLeaseDuration>{seconds}</NewLeaseDuration>"
            ),
        )?;
        return Ok(public);
    }
    Err("todos los puertos cercanos están ocupados en el router".into())
}

/// Diagnóstico (solo lee): dirección de control del router y mi IP local.
pub fn router() -> Result<(String, String)> {
    control()
}

/// Busca el router en la red (SSDP) y la dirección para pedirle cosas; y mi IP local hacia él.
fn control() -> Result<(String, String)> {
    let s = UdpSocket::bind("0.0.0.0:0")?;
    s.set_read_timeout(Some(WAIT))?;
    let pregunta = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\n\
                    ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n";
    s.send_to(pregunta.as_bytes(), "239.255.255.250:1900")?;
    let mut b = [0u8; 2048];
    let (n, _) = s.recv_from(&mut b).map_err(|_| "no hay router con UPnP en esta red")?;
    let resp = String::from_utf8_lossy(&b[..n]);
    let location = resp
        .lines()
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("location")).map(|(_, v)| v.trim().to_string()))
        .ok_or("el router no dice dónde está")?;
    let (host, path) = split_url(&location)?;
    let xml = http(&host, "GET", &path, &[], "")?;
    let after = xml.split(&format!("{SERVICIO}</serviceType>")).nth(1).ok_or("el router no permite abrir puertos")?;
    let ctl = after.split("<controlURL>").nth(1).and_then(|r| r.split('<').next()).ok_or("el router no permite abrir puertos")?;
    let ctl = if ctl.starts_with("http") { split_url(ctl)?.1 } else { ctl.to_string() };
    let local = TcpStream::connect_timeout(&host.to_socket_addrs()?.next().ok_or("router sin dirección")?, WAIT)?.local_addr()?.ip().to_string();
    Ok((format!("{host}{ctl}"), local))
}

/// «http://host:puerto/ruta» → («host:puerto», «/ruta»).
fn split_url(url: &str) -> Result<(String, String)> {
    let rest = url.strip_prefix("http://").ok_or("dirección del router no válida")?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = if host.contains(':') { host.to_string() } else { format!("{host}:80") };
    Ok((host, format!("/{path}")))
}

fn soap(control: &str, action: &str, body: &str) -> Result<String> {
    let (host, path) = control.split_once('/').map(|(h, p)| (h.to_string(), format!("/{p}"))).ok_or("control no válido")?;
    let envelope = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{SERVICIO}\">\
         {body}</u:{action}></s:Body></s:Envelope>"
    );
    let accion = format!("SOAPAction: \"{SERVICIO}#{action}\"");
    http(&host, "POST", &path, &["Content-Type: text/xml; charset=\"utf-8\"", &accion], &envelope)
}

/// HTTP/1.0 mínimo (el router de casa no habla otra cosa). Error si no responde 200.
fn http(host: &str, method: &str, path: &str, headers: &[&str], body: &str) -> Result<String> {
    let addr = host.to_socket_addrs()?.next().ok_or("router sin dirección")?;
    let mut s = TcpStream::connect_timeout(&addr, WAIT)?;
    s.set_read_timeout(Some(WAIT))?;
    let extra: String = headers.iter().map(|h| format!("{h}\r\n")).collect();
    // De una sola vez: algunos routers leen solo el primer paquete y cortan si llega a trozos.
    let req = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    s.write_all(req.as_bytes())?;
    // Algunos routers cortan de golpe (RST) al terminar: vale lo que llegó hasta ahí.
    let mut raw = Vec::new();
    let mut buf = [0u8; 8192];
    while raw.len() < 256 * 1024 {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&buf[..n]),
            Err(e) if raw.is_empty() => return Err(e.into()),
            Err(_) => break,
        }
    }
    let resp = String::from_utf8_lossy(&raw).into_owned();
    if !resp.split_whitespace().nth(1).is_some_and(|c| c == "200") {
        return Err(format!("el router no aceptó {method} {path}").into());
    }
    Ok(resp)
}
