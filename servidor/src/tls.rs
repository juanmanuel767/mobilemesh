// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Juan Manuel Peralta Chacon (GitHub: juanmanuel767)
//! Candado (wss://) para el puente web del relé (pieza aparte). Las páginas https solo pueden
//! conectar con `wss://`: aquí se quita el TLS y los bytes pasan, ya en claro, al puente WebSocket
//! de siempre por una conexión local. El contenido sigue cifrado de extremo a extremo por dentro.
//!
//! El certificado se lee en cada conexión: al renovarlo (cada 60–90 días) no hace falta reiniciar.

use mobilemesh_core::Result;
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use std::{
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

/// Lee el certificado (cadena completa) y su clave en PEM.
pub fn config(cert: &str, key: &str) -> Result<Arc<ServerConfig>> {
    let certs = CertificateDer::pem_file_iter(cert)?.collect::<std::result::Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_file(key)?;
    let cfg = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    Ok(Arc::new(cfg))
}

type Conexion = Arc<Mutex<StreamOwned<ServerConnection, TcpStream>>>;

fn lock(t: &Conexion) -> std::sync::MutexGuard<'_, StreamOwned<ServerConnection, TcpStream>> {
    t.lock().unwrap_or_else(|e| e.into_inner())
}

/// Quita el TLS de `tcp` y empalma con el puente en claro de `plain`.
pub fn terminate(tcp: TcpStream, cfg: Arc<ServerConfig>, plain: SocketAddr) -> Result<()> {
    tcp.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut tls = StreamOwned::new(ServerConnection::new(cfg)?, tcp);
    while tls.conn.is_handshaking() {
        tls.conn.complete_io(&mut tls.sock)?;
    }
    // Lecturas cortas: así quien escribe no espera más de 20 ms a que se suelte el candado.
    tls.sock.set_read_timeout(Some(Duration::from_millis(20)))?;
    let interior = TcpStream::connect_timeout(&plain, Duration::from_secs(5))?;
    interior.set_nodelay(true)?;
    let (mut interior_r, mut interior_w) = (interior.try_clone()?, interior);
    let tls = Arc::new(Mutex::new(tls));
    // El lector suelta y vuelve a coger el candado sin parar: sin este aviso, quien escribe podía
    // esperar segundos su turno (medido: hasta 4 s).
    let escribiendo = Arc::new(AtomicBool::new(false));

    // Navegador → puente.
    let (t, espera) = (tls.clone(), escribiendo.clone());
    let subida = thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        loop {
            while espera.load(Ordering::Acquire) {
                thread::yield_now();
            }
            let r = lock(&t).read(&mut buf);
            match r {
                Ok(0) => break,
                Ok(n) => {
                    if interior_w.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
                Err(_) => break,
            }
        }
        let _ = interior_w.shutdown(Shutdown::Both);
    });
    // Puente → navegador.
    let mut buf = [0u8; 16 * 1024];
    while let Ok(n @ 1..) = interior_r.read(&mut buf) {
        escribiendo.store(true, Ordering::Release);
        let mut g = lock(&tls);
        escribiendo.store(false, Ordering::Release);
        if g.write_all(&buf[..n]).and_then(|_| g.flush()).is_err() {
            break;
        }
    }
    {
        let mut g = lock(&tls);
        g.conn.send_close_notify();
        let _ = g.flush();
        let _ = g.sock.shutdown(Shutdown::Both);
    }
    let _ = subida.join();
    Ok(())
}
