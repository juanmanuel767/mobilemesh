//! El registro CSV del piloto de campo: que escriba lo que debe, que se pueda apagar y que no
//! estorbe. Lo que se comprueba aquí es el archivo, no la red.
//!
//! Va todo en una prueba a propósito: el destino del CSV es un estado global del proceso, y dos
//! pruebas en paralelo se pisarían.

use mobilemesh_core::metricas::{self, CABECERA, Fila};

fn temporal(nombre: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mm-metricas-{}-{nombre}.csv", std::process::id()))
}

fn lineas(p: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(p).unwrap().lines().map(String::from).collect()
}

#[test]
fn el_csv_del_piloto() {
    let csv = temporal("piloto");
    let _ = std::fs::remove_file(&csv);

    // Apagado: ni escribe ni crea el archivo.
    assert!(!metricas::activa(), "las medidas arrancan apagadas");
    metricas::registrar(Fila { evento: "envio", resultado: "directo", ..Default::default() });
    assert!(!csv.exists(), "apagado no puede crear archivos");

    // Encendido: se crea con la cabecera.
    metricas::activar(csv.to_str().unwrap()).unwrap();
    assert!(metricas::activa());
    assert_eq!(metricas::ruta_actual().as_deref(), Some(csv.as_path()));
    assert_eq!(lineas(&csv), vec![CABECERA.to_string()]);

    // Cada suceso es una línea con tantas columnas como la cabecera.
    metricas::bateria(64, false);
    metricas::registrar(Fila {
        evento: "envio",
        resultado: "directo",
        objetivo: metricas::corto("0123456789abcdef"),
        latencia_ms: Some(142),
        n: Some(3),
        ..Default::default()
    });
    metricas::registrar(Fila {
        evento: "supernodo",
        resultado: "cambia",
        objetivo: "203.0.113.7:9000",
        n: Some(4),
        score: Some(0.8125),
        nota: "mejor=203.0.113.7:9000 puntos=0.8125",
        ..Default::default()
    });

    let l = lineas(&csv);
    assert_eq!(l.len(), 3, "cabecera + dos sucesos");
    let columnas = CABECERA.split(',').count();
    for fila in &l[1..] {
        assert_eq!(fila.split(',').count(), columnas, "columnas descuadradas en «{fila}»");
    }

    let envio: Vec<&str> = l[1].split(',').collect();
    assert_eq!(envio[1], "envio");
    assert_eq!(envio[2], "directo");
    assert_eq!(envio[3], "01234567", "del destinatario solo van ocho caracteres");
    assert_eq!(envio[4], "142");
    assert_eq!(envio[5], "3");
    assert_eq!(envio[6], "64", "la batería viaja en cada fila");
    assert_eq!(envio[7], "0");
    assert!(envio[0].parse::<u64>().unwrap() > 1_600_000_000_000, "la marca de tiempo es en milisegundos");

    let sn: Vec<&str> = l[2].split(',').collect();
    assert_eq!(sn[1], "supernodo");
    assert_eq!(sn[2], "cambia");
    assert_eq!(sn[8], "0.8125");

    // Apagado: deja de escribir y el archivo se queda como estaba.
    metricas::desactivar();
    assert!(!metricas::activa());
    metricas::registrar(Fila { evento: "envio", resultado: "fallo", ..Default::default() });
    assert_eq!(lineas(&csv).len(), 3, "apagado no añade nada");

    // Volver a encender sobre el mismo archivo conserva lo anterior y no repite la cabecera.
    metricas::activar(csv.to_str().unwrap()).unwrap();
    metricas::registrar(Fila { evento: "recogida", resultado: "vacio", n: Some(0), ..Default::default() });
    let l = lineas(&csv);
    assert_eq!(l.len(), 4);
    assert_eq!(l[0], CABECERA, "la cabecera se escribe una sola vez");

    // Una ruta imposible (una carpeta en vez de un archivo) no puede tumbar un envío: se pierde la
    // medida y nada más.
    let dir = std::env::temp_dir().join(format!("mm-metricas-dir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    metricas::activar(dir.to_str().unwrap()).unwrap();
    metricas::registrar(Fila { evento: "envio", resultado: "directo", ..Default::default() });

    metricas::desactivar();
    metricas::bateria(0, false);
    let _ = std::fs::remove_file(&csv);
    let _ = std::fs::remove_dir_all(&dir);
}
