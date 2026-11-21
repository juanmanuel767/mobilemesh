//! Carreras de «comprobar y luego escribir»: se simulan llamando al Store en el orden exacto en que
//! ocurren (sin hilos, para que no sean frágiles). La condición vive en el SQL, no en la comprobación previa.

use mobilemesh_core::{Identity, Store, identity::Plain};

fn tienda(nombre: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("mm-carreras-{}-{nombre}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let id = Identity::create(&dir, &Plain).unwrap();
    Store::open(&dir, id.db_key()).unwrap()
}

#[test]
fn documento_publico_no_se_sobrescribe_por_otra_cuenta() {
    let store = tienda("publico");
    assert!(store.public_put("app", "col", "d1", "cuenta-a", r#"{"v":"de A"}"#).unwrap());
    // B ya pasó la comprobación previa (vio None) y escribe después de A.
    assert!(!store.public_put("app", "col", "d1", "cuenta-b", r#"{"v":"de B"}"#).unwrap());
    let docs = store.public_docs("app", "col").unwrap();
    assert_eq!(docs, vec![("d1".to_string(), "cuenta-a".to_string(), r#"{"v":"de A"}"#.to_string())]);
    // El autor sí puede actualizar lo suyo.
    assert!(store.public_put("app", "col", "d1", "cuenta-a", r#"{"v":"nuevo"}"#).unwrap());
    assert_eq!(store.public_docs("app", "col").unwrap()[0].2, r#"{"v":"nuevo"}"#);
}

#[test]
fn borrado_publico_solo_por_su_autor() {
    let store = tienda("borrado");
    store.public_put("app", "col", "d1", "cuenta-a", "{}").unwrap();
    assert_eq!(store.public_delete("app", "col", "d1", "cuenta-b").unwrap(), 0);
    assert_eq!(store.public_author("app", "col", "d1").unwrap().as_deref(), Some("cuenta-a"));
    assert_eq!(store.public_delete("app", "col", "d1", "cuenta-a").unwrap(), 1);
    assert_eq!(store.public_author("app", "col", "d1").unwrap(), None);
    // Borrar lo que no existe borra 0 filas (el relé responde Ack igualmente).
    assert_eq!(store.public_delete("app", "col", "d1", "cuenta-a").unwrap(), 0);
}

#[test]
fn nombre_lo_gana_quien_lo_reclama_primero() {
    let store = tienda("nombre");
    assert!(store.claim_setting("nombre:app/ana", "cuenta-a\ninvitacion-a").unwrap());
    assert!(!store.claim_setting("nombre:app/ana", "cuenta-b\ninvitacion-b").unwrap());
    assert_eq!(store.setting("nombre:app/ana").unwrap().as_deref(), Some("cuenta-a\ninvitacion-a"));
    // A actualiza su propia invitación con set_setting.
    store.set_setting("nombre:app/ana", Some("cuenta-a\ninvitacion-nueva")).unwrap();
    assert_eq!(store.setting("nombre:app/ana").unwrap().as_deref(), Some("cuenta-a\ninvitacion-nueva"));
}
