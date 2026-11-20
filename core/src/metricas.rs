//! Mediciones del piloto de campo: una línea CSV por suceso.
//!
//! Sirve para responder con datos, y no con estimaciones, a tres preguntas del trabajo de grado:
//! cuánto tarda un envío, si acabó en entrega directa o en el buzón, y con qué supernodo se quedó
//! cada teléfono y por qué. Sin esto el núcleo funciona igual pero no deja rastro que medir.
//!
//! Está **apagado mientras nadie diga dónde escribir**: ni crea archivos ni cuesta nada. Se
//! enciende desde la app ([`crate`] lo expone como `set_metrics_path`) o, en el escritorio, con la
//! variable de entorno `MOBILEMESH_METRICAS=/ruta/medidas.csv`.
//!
//! Ninguna medida puede estropear un envío: [`registrar`] no devuelve error ni entra en pánico, y
//! si el archivo no se puede escribir simplemente se pierde esa línea.
//!
//! **Qué no se guarda:** ni mensajes, ni claves, ni nombres de contacto. Del destinatario solo van
//! los primeros ocho caracteres de su identificador, lo justo para distinguir dos teléfonos en una
//! misma prueba.

/// Las columnas del CSV, en orden. Se escribe sola la primera vez que se crea el archivo.
pub const CABECERA: &str = "ts_ms,evento,resultado,objetivo,latencia_ms,n,bateria,cargando,score,rtt_ms,nota";

/// Un suceso medido. Lo que no aplique se deja vacío: `Fila { evento: "envio", ..Default::default() }`.
#[derive(Default, Debug)]
pub struct Fila<'a> {
    /// Qué pasó: `envio`, `recogida`, `supernodo`, `rtt`.
    pub evento: &'a str,
    /// Cómo acabó: `directo`, `buzon`, `fallo`, `mantiene`, `cambia`, `primero`.
    pub resultado: &'a str,
    /// Sobre quién: el supernodo, o los ocho primeros caracteres del destinatario.
    pub objetivo: &'a str,
    /// Lo que tardó la operación completa.
    pub latencia_ms: Option<u64>,
    /// Cuántos: mensajes entregados, cartas recogidas, supernodos en la lista.
    pub n: Option<usize>,
    /// Puntuación del supernodo elegido (0 a 1).
    pub score: Option<f32>,
    /// Ida y vuelta medida, en milisegundos.
    pub rtt_ms: Option<u32>,
    /// Texto libre para el análisis posterior; se escapa solo.
    pub nota: &'a str,
}

/// Batería de este teléfono tal y como la última vez que la app la dijo: nivel (0-100) en el byte
/// bajo y «enchufado» en el bit 8.
///
/// Es una copia de lo que guarda la capa FFI. Se duplica a propósito: aquello solo existe en móvil
/// y PC, y las medidas tienen que poder escribirse desde cualquier parte del núcleo sin arrastrar
/// esa condición a cada llamada.
static BATERIA: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// Apunta cómo está la batería, para que las filas siguientes la lleven. Lo llama la capa FFI cada
/// vez que la app lo informa.
pub fn bateria(nivel: u8, cargando: bool) {
    let v = u16::from(nivel.min(100)) | if cargando { 1 << 8 } else { 0 };
    BATERIA.store(v, std::sync::atomic::Ordering::SeqCst);
}

/// (nivel, enchufado) tal y como lo dejó [`bateria`].
fn bateria_actual() -> (u8, bool) {
    let v = BATERIA.load(std::sync::atomic::Ordering::SeqCst);
    ((v & 0xff).min(100) as u8, v & (1 << 8) != 0)
}

/// Milisegundos desde 1970. Vale 0 si el reloj del aparato está antes de esa fecha.
fn ahora_ms() -> u64 {
    crate::tiempo::SystemTime::now()
        .duration_since(crate::tiempo::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Un campo listo para el CSV: si lleva coma, comilla o salto de línea va entrecomillado y con las
/// comillas dobladas, como manda el formato.
fn escapar(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains([',', '"', '\n', '\r']) {
        std::borrow::Cow::Owned(format!("\"{}\"", s.replace('"', "\"\"")))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// Los ocho primeros caracteres de un identificador: suficiente para distinguir dos teléfonos en
/// una prueba y no suficiente para señalar a una persona.
pub fn corto(id: &str) -> &str {
    let fin = id.char_indices().nth(8).map_or(id.len(), |(i, _)| i);
    &id[..fin]
}

/// La línea de una fila, sin salto final.
fn linea(f: &Fila) -> String {
    let (bat, cargando) = bateria_actual();
    let num = |v: Option<u64>| v.map_or(String::new(), |x| x.to_string());
    format!(
        "{},{},{},{},{},{},{},{},{},{},{}",
        ahora_ms(),
        escapar(f.evento),
        escapar(f.resultado),
        escapar(f.objetivo),
        num(f.latencia_ms),
        num(f.n.map(|x| x as u64)),
        bat,
        u8::from(cargando),
        f.score.map_or(String::new(), |x| format!("{x:.4}")),
        num(f.rtt_ms.map(u64::from)),
        escapar(f.nota),
    )
}

/// Anota el suceso. No hace nada si las medidas están apagadas, y nunca falla: una medida perdida
/// es preferible a un envío roto.
pub fn registrar(f: Fila) {
    if !activa() {
        return;
    }
    escribir(&linea(&f));
}

#[cfg(not(target_arch = "wasm32"))]
mod archivo {
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, Once};

    /// Dónde se escribe. `None` es «apagado», que es como arranca.
    static RUTA: Mutex<Option<PathBuf>> = Mutex::new(None);
    /// La variable de entorno se mira una sola vez, la primera que alguien pregunta.
    static ENTORNO: Once = Once::new();

    /// Lee `MOBILEMESH_METRICAS` la primera vez. Así una prueba de escritorio se enciende sin tocar
    /// el código, y la app no paga nada por ello.
    fn desde_entorno() {
        ENTORNO.call_once(|| {
            if let Ok(p) = std::env::var("MOBILEMESH_METRICAS")
                && !p.is_empty()
            {
                let _ = super::activar(&p);
            }
        });
    }

    /// Empieza a medir en `ruta`. Si el archivo no existe lo crea con la cabecera; si ya existe se
    /// añade al final, para que varias sesiones de prueba queden en el mismo sitio.
    pub fn activar(ruta: &str) -> std::io::Result<()> {
        ENTORNO.call_once(|| {}); // ya decidido: una ruta explícita gana a la del entorno
        let p = PathBuf::from(ruta);
        if let Some(padre) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(padre)?;
        }
        if !p.exists() {
            std::fs::write(&p, format!("{}\n", super::CABECERA))?;
        }
        *RUTA.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
        Ok(())
    }

    /// Deja de medir. El archivo se queda donde está.
    pub fn desactivar() {
        *RUTA.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// ¿Hay dónde escribir?
    pub fn activa() -> bool {
        desde_entorno();
        RUTA.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    /// El archivo que se está midiendo, si hay alguno.
    pub fn ruta_actual() -> Option<PathBuf> {
        desde_entorno();
        RUTA.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Añade la línea. Se traga cualquier error: medir no puede estropear lo medido.
    pub fn escribir(linea: &str) {
        let Some(p) = ruta_actual() else { return };
        let _ = añadir(&p, linea);
    }

    fn añadir(p: &Path, linea: &str) -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
        writeln!(f, "{linea}")
    }
}

#[cfg(target_arch = "wasm32")]
mod archivo {
    //! En el navegador no hay sistema de archivos: las medidas quedan apagadas y [`super::registrar`]
    //! no hace nada. El núcleo compila igual y las llamadas no necesitan condiciones.

    pub fn activa() -> bool {
        false
    }

    pub fn escribir(_linea: &str) {}
}

use archivo::escribir;
#[cfg(not(target_arch = "wasm32"))]
pub use archivo::{activar, desactivar, ruta_actual};
pub use archivo::activa;

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn escapa_solo_lo_que_hace_falta() {
        assert_eq!(escapar("simple"), "simple");
        assert_eq!(escapar("con,coma"), "\"con,coma\"");
        assert_eq!(escapar("con\"comilla"), "\"con\"\"comilla\"");
    }

    #[test]
    fn el_identificador_se_corta_a_ocho() {
        assert_eq!(corto("0123456789abcdef"), "01234567");
        assert_eq!(corto("corto"), "corto");
        assert_eq!(corto(""), "");
    }

    #[test]
    fn la_linea_tiene_tantas_columnas_como_la_cabecera() {
        let f = Fila { evento: "envio", resultado: "directo", objetivo: "abcd1234", latencia_ms: Some(120), n: Some(2), ..Default::default() };
        assert_eq!(linea(&f).split(',').count(), CABECERA.split(',').count());
    }

    #[test]
    fn la_bateria_viaja_en_la_fila() {
        bateria(87, true);
        let l = linea(&Fila { evento: "envio", ..Default::default() });
        let c: Vec<&str> = l.split(',').collect();
        assert_eq!(c[6], "87");
        assert_eq!(c[7], "1");
        bateria(0, false);
    }
}
