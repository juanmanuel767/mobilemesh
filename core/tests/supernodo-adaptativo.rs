//! Selección adaptativa de nodos de apoyo: elegir el supernodo por calidad del enlace, batería y
//! carga, en vez de por sorteo. Aquí se prueba la decisión en sí, sin red: los tiempos de ida y
//! vuelta se dan medidos.


use mobilemesh_core::relay::{MEJORA_MINIMA, SupernodeInfo, cambiar, decidir_supernodo, mejor_con, parse_info, score};

/// Un supernodo con lo que anuncia.
fn sn(addr: &str, bat: u8, cargando: bool, uso: u16) -> SupernodeInfo {
    SupernodeInfo { addr: addr.to_string(), bat, cargando, uso }
}

#[test]
fn entre_dos_iguales_gana_el_de_menor_latencia() {
    let lista = [sn("1.1.1.1:7900", 80, false, 0), sn("2.2.2.2:7900", 80, false, 0)];
    let (elegido, _) = mejor_con(&lista, &[Some(400), Some(20)], "buzon").unwrap();
    assert_eq!(elegido, "2.2.2.2:7900");
}

#[test]
fn entre_dos_iguales_gana_el_de_mas_bateria() {
    let lista = [sn("1.1.1.1:7900", 20, false, 0), sn("2.2.2.2:7900", 90, false, 0)];
    let (elegido, _) = mejor_con(&lista, &[Some(50), Some(50)], "buzon").unwrap();
    assert_eq!(elegido, "2.2.2.2:7900");
}

#[test]
fn un_supernodo_saturado_no_se_elige_si_hay_otro() {
    let lista = [sn("1.1.1.1:7900", 90, false, 64), sn("2.2.2.2:7900", 90, false, 2)];
    let (elegido, _) = mejor_con(&lista, &[Some(50), Some(50)], "buzon").unwrap();
    assert_eq!(elegido, "2.2.2.2:7900");
}

#[test]
fn el_que_no_contesta_no_se_elige() {
    let lista = [sn("1.1.1.1:7900", 100, true, 0), sn("2.2.2.2:7900", 10, false, 40)];
    let (elegido, _) = mejor_con(&lista, &[None, Some(300)], "buzon").unwrap();
    assert_eq!(elegido, "2.2.2.2:7900");
    // Si no contesta ninguno, no hay a quién ir.
    assert!(mejor_con(&lista, &[None, None], "buzon").is_none());
}

#[test]
fn entre_empatados_decide_el_sorteo_y_no_se_amontonan_todos_en_el_mismo() {
    // Cuatro supernodos idénticos: cada buzón debe caer en el suyo según su secreto, como antes.
    let lista: Vec<SupernodeInfo> = (1..=4).map(|i| sn(&format!("{i}.{i}.{i}.{i}:7900"), 80, false, 1)).collect();
    let rtts = [Some(30), Some(30), Some(30), Some(30)];
    let elegidos: std::collections::HashSet<String> =
        ["buzon-a", "buzon-b", "buzon-c", "buzon-d", "buzon-e", "buzon-f"].iter().map(|s| mejor_con(&lista, &rtts, s).unwrap().0).collect();
    assert!(elegidos.len() > 1, "todos los buzones fueron al mismo supernodo: {elegidos:?}");
}

#[test]
fn una_mejora_pequena_no_hace_cambiar_de_supernodo() {
    let mut aspirante = None;
    // El aspirante gana, pero por menos del 20 %: no compensa.
    let quedo = decidir_supernodo(Some(("actual", 0.80)), ("otro", 0.90), &mut aspirante);
    assert_eq!(quedo, "actual");
    assert!(!cambiar(0.80, 0.90));
    assert!(cambiar(0.80, 0.80 * MEJORA_MINIMA + 0.01));
}

#[test]
fn una_mejora_clara_cambia_pero_solo_tras_dos_vueltas_seguidas() {
    let mut aspirante = None;
    // Primera vuelta: gana claro, pero todavía no se cambia.
    assert_eq!(decidir_supernodo(Some(("actual", 0.40)), ("otro", 0.90), &mut aspirante), "actual");
    // Segunda vuelta seguida: ahora sí.
    assert_eq!(decidir_supernodo(Some(("actual", 0.40)), ("otro", 0.90), &mut aspirante), "otro");
}

#[test]
fn un_aspirante_distinto_cada_vuelta_no_llega_a_cambiar_nada() {
    let mut aspirante = None;
    assert_eq!(decidir_supernodo(Some(("actual", 0.40)), ("uno", 0.90), &mut aspirante), "actual");
    assert_eq!(decidir_supernodo(Some(("actual", 0.40)), ("otro", 0.90), &mut aspirante), "actual");
    assert_eq!(decidir_supernodo(Some(("actual", 0.40)), ("uno", 0.90), &mut aspirante), "actual");
}

#[test]
fn sin_supernodo_previo_se_coge_el_mejor_de_una() {
    let mut aspirante = None;
    assert_eq!(decidir_supernodo(None, ("uno", 0.30), &mut aspirante), "uno");
}

#[test]
fn se_entienden_las_metricas_que_da_el_director() {
    let s = parse_info("1.2.3.4:7900>192.168.1.9:7900;b=80;c=1;u=3").unwrap();
    assert_eq!(s.addr, "1.2.3.4:7900>192.168.1.9:7900");
    assert_eq!((s.bat, s.cargando, s.uso), (80, true, 3));
    // Un supernodo antiguo no manda métricas: se le supone lo peor, no lo mejor.
    let viejo = parse_info("1.2.3.4:7900").unwrap();
    assert_eq!((viejo.bat, viejo.cargando, viejo.uso), (0, false, 64));
    assert!(score(&viejo, Some(0)) < score(&s, Some(0)));
    // Basura: ni se cuela ni rompe.
    assert!(parse_info("  ").is_none());
    let raro = parse_info("1.2.3.4:7900;b=zzz;u=999999;x").unwrap();
    assert_eq!((raro.bat, raro.uso), (0, 64));
}
