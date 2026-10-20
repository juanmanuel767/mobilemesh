//! Supernodo: el móvil de un usuario hace de relé ligero para otros mientras está en wifi y con
//! batería (lo decide la app). Atiende lo mismo que un relé para conectar dos dispositivos (QUIEN,
//! buzones y llamadas); las cartas no: se contesta «NO» y van al relé del proyecto, porque un
//! supernodo puede irse en cualquier momento. No puede leer nada: todo va cifrado de punta a punta.
//!
//! Se apunta cada minuto en el director (el relé del proyecto), que comprueba desde fuera que se
//! le llega antes de darlo a otros.

use crate::{
    Result,
    identity::{from_hex, from_hex_any, to_hex},
    relay::{CONTEXT, WAIT, id_of, read_line, read_line_max},
};
use ed25519_dalek::{Signature, VerifyingKey};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Shutdown, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

/// Topes pequeños: es el móvil de alguien, no un servidor.
const MAX_CONEXIONES: usize = 64;
const MAX_POR_IP: usize = 8;
/// La conexión de control de un buzón manda «PING» cada 25 s.
const SILENCIO: Duration = Duration::from_secs(90);
/// Una llamada sin tráfico durante este tiempo se cierra.
const QUIETA: Duration = Duration::from_secs(300);

#[derive(Default)]
struct Estado {
    /// buzón → conexión de control de su dueño
    buzones: HashMap<String, TcpStream>,
    /// ficha → conexión de quien llamó, esperando a que el dueño acepte
    esperando: HashMap<String, TcpStream>,
    /// conexiones abiertas por IP
    conexiones: HashMap<IpAddr, usize>,
}

/// Un supernodo en marcha. Se para con [`Supernodo::parar`] (las llamadas en curso terminan solas).
#[derive(Clone)]
pub struct Supernodo {
    estado: Arc<Mutex<Estado>>,
    activo: Arc<AtomicBool>,
    /// Llamadas empalmadas y bytes reenviados desde que arrancó (solo cuentas, nada de quién).
    llamadas: Arc<AtomicU64>,
    bytes: Arc<AtomicU64>,
}

impl Supernodo {
    /// Atiende en `l` hasta que se pare.
    pub fn arrancar(l: TcpListener) -> Result<Supernodo> {
        l.set_nonblocking(true)?;
        let sn = Supernodo { estado: Arc::default(), activo: Arc::new(AtomicBool::new(true)), llamadas: Arc::default(), bytes: Arc::default() };
        let yo = sn.clone();
        thread::spawn(move || {
            while yo.activo.load(Ordering::SeqCst) {
                match l.accept() {
                    Ok((s, a)) => yo.entrar(s, a.ip()),
                    Err(_) => thread::sleep(Duration::from_millis(200)),
                }
            }
        });
        Ok(sn)
    }

    pub fn parar(&self) {
        self.activo.store(false, Ordering::SeqCst);
        let mut st = self.lock();
        for (_, s) in st.buzones.drain() {
            let _ = s.shutdown(Shutdown::Both);
        }
        st.esperando.clear();
    }

    pub fn activo(&self) -> bool {
        self.activo.load(Ordering::SeqCst)
    }

    /// «N buzones · N llamadas · N KB reenviados».
    pub fn resumen(&self) -> String {
        format!(
            "{} buzones · {} llamadas · {} KB reenviados",
            self.lock().buzones.len(),
            self.llamadas.load(Ordering::Relaxed),
            self.bytes.load(Ordering::Relaxed) / 1024
        )
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn entrar(&self, s: TcpStream, ip: IpAddr) {
        {
            let mut st = self.lock();
            let total: usize = st.conexiones.values().sum();
            let mias = st.conexiones.get(&ip).copied().unwrap_or(0);
            if total >= MAX_CONEXIONES || mias >= MAX_POR_IP {
                return; // se cierra sin contestar
            }
            st.conexiones.insert(ip, mias + 1);
        }
        let yo = self.clone();
        thread::spawn(move || {
            let _ = s.set_nonblocking(false);
            let _ = yo.atender(s, ip);
            let mut st = yo.lock();
            if let Some(n) = st.conexiones.get_mut(&ip) {
                *n -= 1;
                if *n == 0 {
                    st.conexiones.remove(&ip);
                }
            }
        });
    }

    fn atender(&self, mut s: TcpStream, ip: IpAddr) -> Result<()> {
        s.set_read_timeout(Some(WAIT))?;
        s.set_write_timeout(Some(WAIT))?;
        let linea = read_line_max(&mut s, 200)?;
        let (orden, arg) = linea.split_once(' ').unwrap_or((&linea, ""));
        match orden {
            "QUIEN" => writeln!(s, "{ip}")?,
            "REGISTRAR" => self.registrar(s, arg)?,
            "MARCAR" => self.marcar(s, arg)?,
            "ACEPTAR" => {
                let quien = self.lock().esperando.remove(arg);
                if let Some(mut quien) = quien {
                    writeln!(quien, "OK")?;
                    self.llamadas.fetch_add(1, Ordering::Relaxed);
                    empalmar(quien, s, &self.bytes);
                }
            }
            // Cartas, avisos y directorio: los da el relé del proyecto.
            _ => writeln!(s, "NO")?,
        }
        Ok(())
    }

    /// El dueño demuestra tener la clave del buzón (reto firmado) y deja abierta la conexión de
    /// control, por la que se le avisa de cada llamada.
    fn registrar(&self, mut s: TcpStream, publica: &str) -> Result<()> {
        let publica: [u8; 32] = from_hex(publica)?.try_into().map_err(|_| "clave de buzón no válida")?;
        let clave = VerifyingKey::from_bytes(&publica).map_err(|_| "clave de buzón no válida")?;
        let mut reto = [0u8; 32];
        getrandom::fill(&mut reto).map_err(|e| e.to_string())?;
        writeln!(s, "RETO {}", to_hex(&reto))?;
        let firma: [u8; 64] = from_hex_any(read_line(&mut s)?.strip_prefix("FIRMA ").ok_or("se esperaba la firma")?)?
            .try_into()
            .map_err(|_| "firma no válida")?;
        clave.verify_strict(&[CONTEXT, &reto].concat(), &Signature::from_bytes(&firma)).map_err(|_| "firma no válida")?;
        let buzon = id_of(&publica);
        writeln!(s, "OK {buzon}")?;
        self.lock().buzones.insert(buzon.clone(), s.try_clone()?);
        // Se queda leyendo los PING; si calla o se cae, se libera el buzón.
        s.set_read_timeout(Some(SILENCIO))?;
        let mut b = [0u8; 64];
        while self.activo() && matches!(s.read(&mut b), Ok(n) if n > 0) {}
        let mut st = self.lock();
        if st.buzones.get(&buzon).is_some_and(|c| c.peer_addr().ok() == s.peer_addr().ok()) {
            st.buzones.remove(&buzon);
        }
        Ok(())
    }

    fn marcar(&self, mut s: TcpStream, buzon: &str) -> Result<()> {
        let mut ficha = [0u8; 16];
        getrandom::fill(&mut ficha).map_err(|e| e.to_string())?;
        let ficha = to_hex(&ficha);
        let dueno = {
            let mut st = self.lock();
            match st.buzones.get(buzon).map(TcpStream::try_clone) {
                Some(Ok(d)) => {
                    st.esperando.insert(ficha.clone(), s.try_clone()?);
                    Some(d)
                }
                _ => None,
            }
        };
        // Fuera del candado: un dueño que no lee no bloquea al resto.
        if !dueno.is_some_and(|mut d| writeln!(d, "LLAMADA {ficha}").is_ok()) {
            self.lock().esperando.remove(&ficha);
            writeln!(s, "NO")?;
            return Ok(());
        }
        thread::sleep(WAIT);
        if let Some(mut s) = self.lock().esperando.remove(&ficha) {
            let _ = writeln!(s, "NO");
        }
        Ok(())
    }
}

/// Copia en los dos sentidos hasta que uno cierre o pase `QUIETA` sin tráfico.
fn empalmar(a: TcpStream, b: TcpStream, bytes: &Arc<AtomicU64>) {
    let tubo = |mut de: TcpStream, mut a: TcpStream, bytes: Arc<AtomicU64>| {
        let _ = de.set_read_timeout(Some(QUIETA));
        let mut buf = [0u8; 16 * 1024];
        while let Ok(n @ 1..) = de.read(&mut buf) {
            if a.write_all(&buf[..n]).is_err() {
                break;
            }
            bytes.fetch_add(n as u64, Ordering::Relaxed);
        }
        let _ = a.shutdown(Shutdown::Both);
        let _ = de.shutdown(Shutdown::Both);
    };
    if let (Ok(a2), Ok(b2)) = (a.try_clone(), b.try_clone()) {
        let b3 = bytes.clone();
        let otro = thread::spawn(move || tubo(a, b2, b3));
        tubo(b, a2, bytes.clone());
        let _ = otro.join();
    }
}

/// Se apunta en el director como supernodo en `puerto` (el público, abierto en el router) y, si se
/// sabe, con su dirección en la wifi (`wifi`, «ip:puerto»), para quien esté en esa misma casa: el
/// router no deja llegar a la IP pública desde dentro. `true` si el director lo aceptó.
pub fn apuntarse(director: &str, puerto: u16, wifi: Option<&str>) -> bool {
    (|| -> Result<bool> {
        let mut s = crate::net::connect(director, WAIT)?;
        s.set_read_timeout(Some(WAIT * 2))?;
        writeln!(s, "SUPERNODO {puerto} {}", wifi.unwrap_or_default())?;
        Ok(read_line(&mut s)? == "OK")
    })()
    .unwrap_or(false)
}
