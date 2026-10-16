//! Conexiones (pieza aparte). En móvil y PC son TCP. En el navegador serán WebSocket hacia el
//! puente del relé, porque un navegador no puede abrir conexiones TCP. El resto del núcleo solo
//! ve una «conexión» (`Stream`) y no sabe cuál de las dos es.

use crate::Result;
use std::{
    io::{self, Read, Write},
    net::TcpStream,
    time::Duration,
};

pub trait Conn: Read + Write + Send {
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()>;
    fn try_clone_conn(&self) -> io::Result<Stream>;
}

pub type Stream = Box<dyn Conn>;

impl Conn for TcpStream {
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, t)
    }

    fn try_clone_conn(&self) -> io::Result<Stream> {
        Ok(Box::new(self.try_clone()?))
    }
}

impl Conn for Stream {
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        (**self).set_read_timeout(t)
    }

    fn try_clone_conn(&self) -> io::Result<Stream> {
        (**self).try_clone_conn()
    }
}

/// Conecta con «host:puerto» (TCP).
#[cfg(not(target_arch = "wasm32"))]
pub fn connect(addr: &str, wait: Duration) -> Result<Stream> {
    use std::net::ToSocketAddrs;
    let a = addr.to_socket_addrs()?.next().ok_or("dirección no válida")?;
    Ok(Box::new(TcpStream::connect_timeout(&a, wait)?))
}

/// En el navegador: WebSocket hacia el puente del relé.
#[cfg(target_arch = "wasm32")]
pub fn connect(addr: &str, wait: Duration) -> Result<Stream> {
    crate::web::connect(addr, wait)
}
