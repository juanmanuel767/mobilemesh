//! Base de datos local (SQLite): contactos, mensajes con su estado y estado del cifrado.
//! El texto de los mensajes y el estado del ratchet se guardan cifrados: una copia del archivo no los revela.

use crate::{
    Result,
    identity::{from_hex, from_hex_any, to_hex},
};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, Payload},
};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use crate::files::FileMeta;
use crate::sync::{FieldState, Op, Vector};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use vodozemac::olm::{Account, AccountPickle, Session, SessionPickle};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS contacts(
  name TEXT PRIMARY KEY,
  key  TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS messages(
  id         TEXT NOT NULL,
  peer       TEXT NOT NULL,              -- clave pública del otro dispositivo
  outgoing   INTEGER NOT NULL,           -- 1 = enviado por mí
  body       TEXT NOT NULL,
  status     TEXT NOT NULL,              -- pendiente | entregado | guardado
  created_at INTEGER NOT NULL DEFAULT (unixepoch()),
  PRIMARY KEY (peer, outgoing, id)       -- un par no puede pisar mensajes de otro
);";

/// Cambios del esquema en orden. La posición en la lista es la versión (PRAGMA user_version).
/// Nunca editar una ya publicada: solo añadir al final.
const MIGRATIONS: &[&str] = &[
    "ALTER TABLE contacts ADD COLUMN addr TEXT;", // 1: dirección ip:puerto del contacto
    // 2: los contactos pasan a identificarse por su cuenta, no por su dispositivo. Las claves
    // guardadas antes ya no significan nada en el protocolo 2: hay que volver a invitarse.
    "DELETE FROM contacts;",
    // 3: cifrado de extremo a extremo (Fase 5).
    "CREATE TABLE olm_account(id INTEGER PRIMARY KEY CHECK (id = 1), pickle TEXT NOT NULL);
     CREATE TABLE olm_sessions(device TEXT NOT NULL, session_id TEXT NOT NULL, pickle TEXT NOT NULL,
                               used INTEGER NOT NULL, PRIMARY KEY (device, session_id));
     -- clave Olm de cada dispositivo, fijada la primera vez: si cambia, se rechaza
     CREATE TABLE olm_devices(device TEXT PRIMARY KEY, identity TEXT NOT NULL);",
    // 4: archivos adjuntos (datos cifrados, con su clave) y progreso de la transferencia (Fase 6).
    "ALTER TABLE messages ADD COLUMN file TEXT;
     ALTER TABLE messages ADD COLUMN progress INTEGER NOT NULL DEFAULT 0;",
    // 5: datos sincronizados entre los dispositivos de la cuenta (Fase 7).
    "CREATE TABLE sync_ops(device TEXT NOT NULL, seq INTEGER NOT NULL, hlc INTEGER NOT NULL,
                           data TEXT NOT NULL, PRIMARY KEY (device, seq));
     CREATE TABLE sync_fields(collection TEXT NOT NULL, record TEXT NOT NULL, field TEXT NOT NULL,
                              value TEXT, hlc INTEGER NOT NULL, device TEXT NOT NULL, seq INTEGER NOT NULL,
                              PRIMARY KEY (collection, record, field));
     CREATE TABLE sync_conflicts(collection TEXT NOT NULL, record TEXT NOT NULL, field TEXT NOT NULL,
                                 device TEXT NOT NULL, seq INTEGER NOT NULL,
                                 PRIMARY KEY (collection, record, field, device, seq));
     -- lo que cada dispositivo propio ha confirmado tener (vector de versiones)
     CREATE TABLE sync_peers(device TEXT PRIMARY KEY, vector TEXT NOT NULL);
     CREATE TABLE sync_meta(k TEXT PRIMARY KEY, v INTEGER NOT NULL);
     -- direcciones de los otros dispositivos de mi cuenta, para sincronizar
     CREATE TABLE my_devices(addr TEXT PRIMARY KEY, name TEXT NOT NULL);",
    // 6: copias cifradas en otros dispositivos (Fase 8).
    "-- cuentas a las que acepto guardarles copia (voluntario)
     CREATE TABLE replica_consent(owner TEXT PRIMARY KEY);
     -- copias que guardo para otros: no las puedo leer
     CREATE TABLE replicas_held(owner TEXT PRIMARY KEY, version INTEGER NOT NULL, meta TEXT NOT NULL,
                                size INTEGER NOT NULL, updated_at INTEGER NOT NULL DEFAULT (unixepoch()));
     -- quién guarda mi copia y en qué estado está
     CREATE TABLE replica_holders(name TEXT PRIMARY KEY, account TEXT NOT NULL, addr TEXT NOT NULL,
                                  version INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'prevista',
                                  checked_at INTEGER);
     -- mi copia más reciente
     CREATE TABLE my_replica(id INTEGER PRIMARY KEY CHECK (id = 1), version INTEGER NOT NULL,
                             meta TEXT NOT NULL, size INTEGER NOT NULL);",
    // 7: ajustes del dispositivo (relé y secreto de su buzón, Fase 9). Los valores van cifrados.
    "CREATE TABLE settings(k TEXT PRIMARY KEY, v TEXT NOT NULL);",
    // 8: permisos (Fase 10).
    "-- lista firmada de dispositivos retirados de cada cuenta conocida (la mía incluida)
     CREATE TABLE revocations(account TEXT PRIMARY KEY, version INTEGER NOT NULL, data TEXT NOT NULL);
     CREATE TABLE blocked(account TEXT PRIMARY KEY);
     -- dispositivos de mi cuenta que he visto (para poder retirarlos)
     CREATE TABLE seen_devices(device TEXT PRIMARY KEY, seen INTEGER NOT NULL DEFAULT (unixepoch()));
     -- colecciones compartidas con contactos: qué puede leer cada uno y si puede cambiarlas
     CREATE TABLE shares(collection TEXT NOT NULL, account TEXT NOT NULL, can_write INTEGER NOT NULL,
                         PRIMARY KEY (collection, account));",
    // 9: cada dispositivo queda ligado a la cuenta que lo usó primero (auditoría, Fase 13): un
    // contacto no puede mandar operaciones con el dispositivo de otro.
    "CREATE TABLE op_authors(device TEXT PRIMARY KEY, author TEXT NOT NULL);",
    // 10: buzón de cartas (avisos en frío): con qué dispositivo de cada contacto hablo y a qué
    // cuenta pertenece cada dispositivo visto.
    "ALTER TABLE contacts ADD COLUMN device TEXT;
     CREATE TABLE device_accounts(device TEXT PRIMARY KEY, account TEXT NOT NULL);",
];

/// Mensaje tal como va dentro de una copia de seguridad.
#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SnapMsg {
    pub id: String,
    pub peer: String,
    pub outgoing: bool,
    pub body: String,
    pub status: String,
    pub file: Option<FileMeta>,
    pub created_at: i64,
}

/// Sin el texto ni las claves: que nunca acaben en un registro por error.
impl std::fmt::Debug for SnapMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SnapMsg({}, {} bytes)", self.id, self.body.len())
    }
}

/// Quien guarda (o va a guardar) mi copia.
#[derive(Debug, PartialEq)]
pub struct Holder {
    pub name: String,
    pub account: String,
    pub addr: String,
    pub version: u64,
    /// prevista | al día | desactualizada | verificada | dañada | sin respuesta | borrando
    pub status: String,
}

/// Sesiones guardadas por dispositivo (si los dos lados abren una a la vez, conviven un tiempo).
const MAX_SESSIONS: i64 = 5;
const SEALED: &str = "e1:";

fn subkey(key: &[u8; 32], label: &[u8]) -> [u8; 32] {
    Sha256::new_with_prefix(label).chain_update(key).finalize().into()
}

pub struct Store {
    db: Connection,
    files: PathBuf,
    replicas: PathBuf,
    body_key: [u8; 32],
    pickle_key: [u8; 32],
}

pub struct Row {
    pub id: String,
    /// Nombre del contacto (o principio de su clave si ya no está).
    pub peer: String,
    /// Clave de la cuenta del contacto.
    pub peer_key: String,
    pub outgoing: bool,
    pub body: String,
    pub status: String,
    pub file: Option<FileMeta>,
    /// Porcentaje transferido de un archivo.
    pub progress: u8,
}

/// Mensaje pendiente de enviar: (identificador, texto, archivo adjunto).
pub type Outgoing = (String, String, Option<FileMeta>);

impl Store {
    /// `key` viene de la identidad del dispositivo (protegida por Keystore).
    pub fn open(dir: &Path, key: [u8; 32]) -> Result<Store> {
        let db = Connection::open(dir.join("mesh.db"))?;
        // Lo borrado se sobrescribe con ceros: no quedan restos del texto en el archivo.
        db.execute_batch("PRAGMA secure_delete = ON;")?;
        db.execute_batch(SCHEMA)?;
        // Todas las migraciones pendientes juntas y con la base reservada: dos aperturas a la vez
        // (la app y su servicio) no pueden aplicar dos veces la misma (encontrado en la auditoría).
        db.execute_batch("BEGIN IMMEDIATE;")?;
        let migrate = || -> Result<()> {
            let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            for (i, m) in MIGRATIONS.iter().enumerate().skip(version as usize) {
                db.execute_batch(&format!("{m} PRAGMA user_version = {};", i + 1))?;
            }
            Ok(())
        };
        match migrate() {
            Ok(()) => db.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = db.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
        let store = Store { db, files: dir.join("archivos"), replicas: dir.join("copias"), body_key: subkey(&key, b"mobilemesh-cuerpos-v1"), pickle_key: subkey(&key, b"mobilemesh-olm-v1") };
        store.seal_old_bodies()?;
        Ok(store)
    }

    /// Los mensajes guardados antes de la Fase 5 estaban en claro: se cifran al abrir.
    fn seal_old_bodies(&self) -> Result<()> {
        let mut st = self.db.prepare("SELECT rowid, id, body FROM messages WHERE body NOT LIKE 'e1:%'")?;
        let old = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        for row in old.collect::<rusqlite::Result<Vec<_>>>()? {
            self.db.execute("UPDATE messages SET body = ?2 WHERE rowid = ?1", params![row.0, self.seal(&row.1, &row.2)?])?;
        }
        Ok(())
    }

    fn seal(&self, id: &str, body: &str) -> Result<String> {
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let ct = ChaCha20Poly1305::new(&self.body_key.into())
            .encrypt(&nonce.into(), Payload { msg: body.as_bytes(), aad: id.as_bytes() })
            .map_err(|_| "no se pudo cifrar el mensaje")?;
        Ok(format!("{SEALED}{}{}", to_hex(&nonce), to_hex(&ct)))
    }

    fn try_unseal(&self, id: &str, stored: &str) -> Option<String> {
        let b = from_hex_any(stored.strip_prefix(SEALED)?).ok()?;
        let (nonce, ct) = b.split_at_checked(12)?;
        let plain = ChaCha20Poly1305::new(&self.body_key.into())
            .decrypt(nonce.into(), Payload { msg: ct, aad: id.as_bytes() })
            .ok()?;
        String::from_utf8(plain).ok()
    }

    /// Para mostrar: si no se puede descifrar (identidad restaurada en este mismo teléfono), lo dice.
    fn unseal(&self, id: &str, stored: &str) -> String {
        self.try_unseal(id, stored).unwrap_or_else(|| "(no se puede descifrar con la identidad actual)".into())
    }

    // ---------- Estado del cifrado de extremo a extremo ----------

    pub fn olm_account(&self) -> Result<Account> {
        let stored: Option<String> = self.db.query_row("SELECT pickle FROM olm_account", [], |r| r.get(0)).optional()?;
        match stored {
            Some(p) => Ok(Account::from_pickle(AccountPickle::from_encrypted(&p, &self.pickle_key)?)),
            None => {
                let account = Account::new();
                self.save_account(&account)?;
                Ok(account)
            }
        }
    }

    pub fn save_account(&self, account: &Account) -> Result<()> {
        self.db.execute(
            "INSERT INTO olm_account(id, pickle) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET pickle = excluded.pickle",
            [account.pickle().encrypt(&self.pickle_key)],
        )?;
        Ok(())
    }

    /// Sesiones con un dispositivo, la usada más recientemente primero.
    pub fn sessions(&self, device: &str) -> Result<Vec<Session>> {
        let mut st = self.db.prepare("SELECT pickle FROM olm_sessions WHERE device = ?1 ORDER BY used DESC")?;
        let pickles = st.query_map([device], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        pickles.iter().map(|p| Ok(Session::from_pickle(SessionPickle::from_encrypted(p, &self.pickle_key)?))).collect()
    }

    pub fn save_session(&self, device: &str, session: &Session) -> Result<()> {
        self.db.execute(
            "INSERT INTO olm_sessions(device, session_id, pickle, used)
             VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(used), 0) + 1 FROM olm_sessions))
             ON CONFLICT(device, session_id) DO UPDATE SET pickle = excluded.pickle, used = excluded.used",
            params![device, session.session_id(), session.pickle().encrypt(&self.pickle_key)],
        )?;
        self.db.execute(
            "DELETE FROM olm_sessions WHERE device = ?1 AND session_id NOT IN
               (SELECT session_id FROM olm_sessions WHERE device = ?1 ORDER BY used DESC LIMIT ?2)",
            params![device, MAX_SESSIONS],
        )?;
        Ok(())
    }

    pub fn drop_sessions(&self, device: &str) -> Result<()> {
        self.db.execute("DELETE FROM olm_sessions WHERE device = ?1", [device])?;
        Ok(())
    }

    pub fn pinned(&self, device: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT identity FROM olm_devices WHERE device = ?1", [device], |r| r.get(0)).optional()?)
    }

    /// Fija la clave Olm de un dispositivo la primera vez; si después llega otra distinta, error.
    pub fn pin(&self, device: &str, identity: &str) -> Result<()> {
        self.db.execute("INSERT OR IGNORE INTO olm_devices(device, identity) VALUES (?1, ?2)", params![device, identity])?;
        match self.pinned(device)? {
            Some(p) if p == identity => Ok(()),
            _ => Err(crate::e2e::KEY_CHANGED.into()),
        }
    }

    /// Añade un contacto o actualiza su dirección. Nunca cambia en silencio la clave de un contacto.
    pub fn add_contact(&self, name: &str, key: &str, addr: Option<&str>) -> Result<()> {
        from_hex(key)?;
        match self.contact_key(name)? {
            Some(k) if k != key => Err(format!(
                "«{name}» ya existe con otra clave; si de verdad cambió, compruébalo con esa persona y añádelo con otro nombre"
            )
            .into()),
            Some(_) => {
                if let Some(a) = addr {
                    self.db.execute("UPDATE contacts SET addr = ?2 WHERE name = ?1", params![name, a])?;
                }
                Ok(())
            }
            None => match self.db.execute("INSERT INTO contacts(name, key, addr) VALUES (?1, ?2, ?3)", params![name, key, addr]) {
                Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                    Err("esa clave ya está en tus contactos con otro nombre".into())
                }
                r => r.map(drop).map_err(Into::into),
            },
        }
    }

    /// Contactos como (nombre, clave, dirección).
    pub fn contacts(&self) -> Result<Vec<(String, String, Option<String>)>> {
        let mut st = self.db.prepare("SELECT name, key, addr FROM contacts ORDER BY name")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn contact_key(&self, name: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT key FROM contacts WHERE name = ?1", [name], |r| r.get(0)).optional()?)
    }

    pub fn contact_name(&self, key: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT name FROM contacts WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn new_id(&self) -> Result<String> {
        Ok(self.db.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?)
    }

    /// Dónde se guarda (cifrado) el archivo de un mensaje. Carpeta privada de la app, nunca pública.
    pub fn blob(&self, peer: &str, outgoing: bool, id: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.files)?;
        Ok(self.files.join(format!("{}-{id}-{}", &peer[..16.min(peer.len())], if outgoing { "sale" } else { "entra" })))
    }

    fn seal_file(&self, id: &str, meta: &FileMeta) -> Result<String> {
        self.seal(&format!("{id}:archivo"), &serde_json::to_string(meta)?)
    }

    fn unseal_file(&self, id: &str, stored: Option<String>) -> Option<FileMeta> {
        serde_json::from_str(&self.unseal(&format!("{id}:archivo"), &stored?)).ok()
    }

    /// Guarda un mensaje saliente (cifrado) como pendiente y devuelve su identificador.
    pub fn queue(&self, peer: &str, body: &str) -> Result<String> {
        let id = self.new_id()?;
        self.db.execute(
            "INSERT INTO messages(id, peer, outgoing, body, status) VALUES (?1, ?2, 1, ?3, 'pendiente')",
            params![id, peer, self.seal(&id, body)?],
        )?;
        Ok(id)
    }

    /// Guarda un archivo saliente ya cifrado en `blob(peer, true, id)`.
    pub fn queue_file(&self, peer: &str, id: &str, meta: &FileMeta) -> Result<()> {
        self.db.execute(
            "INSERT INTO messages(id, peer, outgoing, body, status, file) VALUES (?1, ?2, 1, ?3, 'pendiente', ?4)",
            params![id, peer, self.seal(id, &meta.name)?, self.seal_file(id, meta)?],
        )?;
        Ok(())
    }

    pub fn pending(&self, peer: &str) -> Result<Vec<Outgoing>> {
        let mut st = self.db.prepare(
            "SELECT id, body, file FROM messages WHERE peer = ?1 AND outgoing = 1 AND status = 'pendiente' ORDER BY rowid",
        )?;
        let rows = st.query_map([peer], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            // Lo que no se puede descifrar no se envía: nunca sale un texto de relleno como si fuera
            // el mensaje (auditoría, Fase 13).
            .filter_map(|(id, b, f)| Some((id.clone(), self.try_unseal(&id, &b)?, self.unseal_file(&id, f))))
            .collect())
    }

    pub fn mark_delivered(&self, peer: &str, id: &str) -> Result<()> {
        self.db.execute(
            "UPDATE messages SET status = 'entregado', progress = 100 WHERE peer = ?1 AND outgoing = 1 AND id = ?2",
            params![peer, id],
        )?;
        Ok(())
    }

    pub fn set_status(&self, peer: &str, outgoing: bool, id: &str, status: &str) -> Result<()> {
        self.db.execute(
            "UPDATE messages SET status = ?4 WHERE peer = ?1 AND outgoing = ?2 AND id = ?3",
            params![peer, outgoing, id, status],
        )?;
        Ok(())
    }

    pub fn set_progress(&self, peer: &str, outgoing: bool, id: &str, done: u64, total: u64) -> Result<()> {
        self.db.execute(
            "UPDATE messages SET progress = ?4 WHERE peer = ?1 AND outgoing = ?2 AND id = ?3",
            params![peer, outgoing, id, (done * 100 / total.max(1)) as i64],
        )?;
        Ok(())
    }

    /// Guarda un mensaje recibido. Si ya existía (reintento), no lo duplica.
    pub fn save_incoming(&self, peer: &str, id: &str, body: &str) -> Result<()> {
        self.db.execute(
            "INSERT OR IGNORE INTO messages(id, peer, outgoing, body, status) VALUES (?1, ?2, 0, ?3, 'guardado')",
            params![id, peer, self.seal(id, body)?],
        )?;
        Ok(())
    }

    /// Registra un archivo que empieza a llegar (o sigue llegando) y devuelve su estado actual.
    pub fn incoming_file(&self, peer: &str, id: &str, meta: &FileMeta) -> Result<String> {
        self.db.execute(
            "INSERT OR IGNORE INTO messages(id, peer, outgoing, body, status, file) VALUES (?1, ?2, 0, ?3, 'recibiendo', ?4)",
            params![id, peer, self.seal(id, &meta.name)?, self.seal_file(id, meta)?],
        )?;
        Ok(self.db.query_row(
            "SELECT status FROM messages WHERE peer = ?1 AND outgoing = 0 AND id = ?2",
            params![peer, id],
            |r| r.get(0),
        )?)
    }

    pub fn file_of(&self, peer: &str, outgoing: bool, id: &str) -> Result<Option<FileMeta>> {
        let f: Option<Option<String>> = self
            .db
            .query_row("SELECT file FROM messages WHERE peer = ?1 AND outgoing = ?2 AND id = ?3", params![peer, outgoing, id], |r| r.get(0))
            .optional()?;
        Ok(self.unseal_file(id, f.flatten()))
    }

    pub fn list(&self) -> Result<Vec<Row>> {
        let mut st = self.db.prepare(
            "SELECT COALESCE(c.name, substr(m.peer, 1, 12)), m.outgoing, m.body, m.status, m.id, m.peer, m.file, m.progress
             FROM messages m LEFT JOIN contacts c ON c.key = m.peer ORDER BY m.created_at, m.rowid",
        )?;
        let rows = st.query_map([], |r| {
            let (body, id): (String, String) = (r.get(2)?, r.get(4)?);
            Ok(Row {
                body: self.unseal(&id, &body),
                file: self.unseal_file(&id, r.get(6)?),
                id,
                peer: r.get(0)?,
                peer_key: r.get(5)?,
                outgoing: r.get(1)?,
                status: r.get(3)?,
                progress: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---------- Datos sincronizados (Fase 7) ----------

    fn meta(&self, k: &str) -> Result<u64> {
        Ok(self.db.query_row("SELECT v FROM sync_meta WHERE k = ?1", [k], |r| r.get::<_, i64>(0)).optional()?.unwrap_or(0) as u64)
    }

    pub(crate) fn set_meta(&self, k: &str, v: u64) -> Result<()> {
        self.db.execute("INSERT INTO sync_meta(k, v) VALUES (?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v", params![k, v as i64])?;
        Ok(())
    }

    /// Siguiente valor del reloj híbrido: nunca retrocede aunque el reloj del teléfono lo haga.
    pub fn next_hlc(&self) -> Result<u64> {
        let h = crate::sync::now_hlc().max(self.meta("hlc")?.saturating_add(1));
        self.set_meta("hlc", h)?;
        Ok(h)
    }

    /// Al recibir un cambio, el reloj propio se adelanta si hace falta: lo hecho después va detrás.
    pub fn observe_hlc(&self, h: u64) -> Result<()> {
        if h > self.meta("hlc")? {
            self.set_meta("hlc", h)?;
        }
        Ok(())
    }

    pub fn next_seq(&self, device: &str) -> Result<u64> {
        Ok(self.max_seq(device)?.checked_add(1).ok_or("número de operación agotado")?)
    }

    pub fn max_seq(&self, device: &str) -> Result<u64> {
        Ok(self.db.query_row("SELECT COALESCE(MAX(seq), 0) FROM sync_ops WHERE device = ?1", [device], |r| r.get::<_, i64>(0))? as u64)
    }

    /// Cuenta a la que pertenece un dispositivo según lo visto hasta ahora (la primera que lo usó).
    pub fn device_author(&self, device: &str) -> Result<Option<String>> {
        let bound: Option<String> = self.db.query_row("SELECT author FROM op_authors WHERE device = ?1", [device], |r| r.get(0)).optional()?;
        if bound.is_some() {
            return Ok(bound);
        }
        // Operaciones de antes de esta tabla: se toma la de cualquier operación ya guardada.
        let seq: Option<i64> = self.db.query_row("SELECT seq FROM sync_ops WHERE device = ?1 LIMIT 1", [device], |r| r.get(0)).optional()?;
        Ok(match seq {
            Some(seq) => self.op(device, seq as u64)?.map(|op| op.author),
            None => None,
        })
    }

    /// Ejecuta `f` como un todo: o se guarda todo o nada.
    pub fn atomic<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        self.db.execute_batch("BEGIN IMMEDIATE;")?;
        match f() {
            Ok(v) => {
                self.db.execute_batch("COMMIT;")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.db.execute_batch("ROLLBACK;");
                Err(e)
            }
        }
    }

    /// Guarda una operación. `false` si ya la teníamos.
    pub fn insert_op(&self, op: &Op) -> Result<bool> {
        let data = self.seal(&format!("op:{}:{}", op.device, op.seq), &serde_json::to_string(op)?)?;
        self.db.execute("INSERT OR IGNORE INTO op_authors(device, author) VALUES (?1, ?2)", params![op.device, op.author])?;
        Ok(self.db.execute(
            "INSERT OR IGNORE INTO sync_ops(device, seq, hlc, data) VALUES (?1, ?2, ?3, ?4)",
            params![op.device, op.seq as i64, op.hlc as i64, data],
        )? == 1)
    }

    fn op(&self, device: &str, seq: u64) -> Result<Option<Op>> {
        let data: Option<String> = self
            .db
            .query_row("SELECT data FROM sync_ops WHERE device = ?1 AND seq = ?2", params![device, seq as i64], |r| r.get(0))
            .optional()?;
        Ok(data.and_then(|d| serde_json::from_str(&self.unseal(&format!("op:{device}:{seq}"), &d)).ok()))
    }

    pub fn vector(&self) -> Result<Vector> {
        let mut st = self.db.prepare("SELECT device, MAX(seq) FROM sync_ops GROUP BY device")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Operaciones que no están cubiertas por `have`, en orden de reloj.
    pub fn ops_after(&self, have: &Vector) -> Result<Vec<Op>> {
        let mut st = self.db.prepare("SELECT device, seq FROM sync_ops ORDER BY hlc, device")?;
        let keys = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::new();
        for (device, seq) in keys {
            if seq > have.get(&device).copied().unwrap_or(0) {
                out.push(self.op(&device, seq)?.ok_or("operación ilegible en la base de datos")?);
            }
        }
        Ok(out)
    }

    /// Vector de versiones solo de unas colecciones, por colección y dispositivo (para contactos).
    pub fn vector_in(&self, cols: &[String]) -> Result<Vector> {
        let mut v = Vector::new();
        for op in self.ops_after(&Vector::new())? {
            if cols.contains(&op.collection) {
                let e = v.entry(format!("{}\u{1}{}", op.collection, op.device)).or_insert(0);
                *e = (*e).max(op.seq);
            }
        }
        Ok(v)
    }

    /// Operaciones de unas colecciones que no cubre `have` (vector de `vector_in`).
    pub fn ops_after_in(&self, have: &Vector, cols: &[String]) -> Result<Vec<Op>> {
        Ok(self
            .ops_after(&Vector::new())?
            .into_iter()
            .filter(|op| {
                cols.contains(&op.collection) && op.seq > have.get(&format!("{}\u{1}{}", op.collection, op.device)).copied().unwrap_or(0)
            })
            .collect())
    }

    pub fn field(&self, collection: &str, record: &str, field: &str) -> Result<Option<FieldState>> {
        let row = self
            .db
            .query_row(
                "SELECT value, hlc, device, seq FROM sync_fields WHERE collection = ?1 AND record = ?2 AND field = ?3",
                params![collection, record, field],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)),
            )
            .optional()?;
        let aad = format!("campo:{collection}/{record}/{field}");
        Ok(row.map(|(v, hlc, device, seq)| FieldState {
            value: v.map(|v| self.unseal(&aad, &v)),
            hlc: hlc as u64,
            device,
            seq: seq as u64,
        }))
    }

    pub fn set_field(&self, collection: &str, record: &str, field: &str, s: &FieldState) -> Result<()> {
        let aad = format!("campo:{collection}/{record}/{field}");
        let value = s.value.as_ref().map(|v| self.seal(&aad, v)).transpose()?;
        self.db.execute(
            "INSERT INTO sync_fields(collection, record, field, value, hlc, device, seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(collection, record, field) DO UPDATE SET
               value = excluded.value, hlc = excluded.hlc, device = excluded.device, seq = excluded.seq",
            params![collection, record, field, value, s.hlc as i64, s.device, s.seq as i64],
        )?;
        Ok(())
    }

    /// Campos de una colección como (registro, campo, estado), ordenados por registro.
    pub fn fields(&self, collection: &str) -> Result<Vec<(String, String, FieldState)>> {
        let mut st = self.db.prepare("SELECT record, field FROM sync_fields WHERE collection = ?1 ORDER BY record, field")?;
        let keys = st.query_map([collection], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::new();
        for (record, field) in keys {
            let state = self.field(collection, &record, &field)?.ok_or("campo desaparecido")?;
            out.push((record, field, state));
        }
        Ok(out)
    }

    pub fn add_conflict(&self, collection: &str, record: &str, field: &str, device: &str, seq: u64) -> Result<()> {
        self.db.execute(
            "INSERT OR IGNORE INTO sync_conflicts(collection, record, field, device, seq) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![collection, record, field, device, seq as i64],
        )?;
        Ok(())
    }

    pub fn clear_conflict(&self, collection: &str, record: &str, field: &str, device: &str, seq: u64) -> Result<()> {
        self.db.execute(
            "DELETE FROM sync_conflicts WHERE collection = ?1 AND record = ?2 AND field = ?3 AND device = ?4 AND seq = ?5",
            params![collection, record, field, device, seq as i64],
        )?;
        Ok(())
    }

    /// Conflictos abiertos de un campo como (dispositivo, nº de operación).
    pub fn conflict_keys(&self, collection: &str, record: &str, field: &str) -> Result<Vec<(String, u64)>> {
        let mut st = self.db.prepare("SELECT device, seq FROM sync_conflicts WHERE collection = ?1 AND record = ?2 AND field = ?3")?;
        let rows = st.query_map(params![collection, record, field], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Valores alternativos (los que perdieron) de un campo en conflicto.
    pub fn conflict_values(&self, collection: &str, record: &str, field: &str) -> Result<Vec<String>> {
        let keys = self.conflict_keys(collection, record, field)?;
        let mut out = Vec::new();
        for (device, seq) in keys {
            if let Some(op) = self.op(&device, seq)? {
                out.push(op.value.unwrap_or_else(|| "(borrado)".into()));
            }
        }
        Ok(out)
    }

    pub fn set_peer_vector(&self, device: &str, v: &Vector) -> Result<()> {
        self.db.execute(
            "INSERT INTO sync_peers(device, vector) VALUES (?1, ?2) ON CONFLICT(device) DO UPDATE SET vector = excluded.vector",
            params![device, serde_json::to_string(v)?],
        )?;
        Ok(())
    }

    pub fn peer_vectors(&self) -> Result<HashMap<String, Vector>> {
        let mut st = self.db.prepare("SELECT device, vector FROM sync_peers")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = HashMap::new();
        for (d, v) in rows.collect::<rusqlite::Result<Vec<_>>>()? {
            out.insert(d, serde_json::from_str(&v)?);
        }
        Ok(out)
    }

    /// Añade otro dispositivo de mi cuenta o, si ya estaba con ese nombre, cambia su dirección.
    pub fn add_my_device(&self, name: &str, addr: &str) -> Result<()> {
        self.db.execute("DELETE FROM my_devices WHERE name = ?1 OR addr = ?2", params![name, addr])?;
        self.db.execute("INSERT INTO my_devices(addr, name) VALUES (?1, ?2)", params![addr, name])?;
        Ok(())
    }

    /// Otros dispositivos de mi cuenta como (nombre, dirección).
    pub fn my_devices(&self) -> Result<Vec<(String, String)>> {
        let mut st = self.db.prepare("SELECT name, addr FROM my_devices ORDER BY name")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn setting(&self, k: &str) -> Result<Option<String>> {
        let v: Option<String> = self.db.query_row("SELECT v FROM settings WHERE k = ?1", [k], |r| r.get(0)).optional()?;
        Ok(v.map(|v| self.unseal(&format!("ajuste:{k}"), &v)))
    }

    pub fn set_setting(&self, k: &str, v: Option<&str>) -> Result<()> {
        match v {
            Some(v) => self.db.execute(
                "INSERT INTO settings(k, v) VALUES (?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
                params![k, self.seal(&format!("ajuste:{k}"), v)?],
            )?,
            None => self.db.execute("DELETE FROM settings WHERE k = ?1", [k])?,
        };
        Ok(())
    }

    /// Relés en los que tengo buzón: los que eligió la app («a|b») o, si no eligió ninguno, dos de
    /// los universales (del directorio o de la lista fija). `SIN_RELE` los quita todos.
    pub fn relays(&self) -> Result<Vec<String>> {
        let split = |r: &str| -> Vec<String> { r.split('|').map(str::trim).filter(|r| !r.is_empty() && *r != crate::relay::SIN_RELE).map(String::from).collect() };
        if let Some(r) = self.setting("rele")? {
            return Ok(split(&r));
        }
        let all = crate::directorio::reles(self);
        if all.is_empty() {
            return Ok(vec![]);
        }
        // Se apuntan para que añadir relés a la lista no cambie la ruta que ya tienen los
        // contactos; solo se eligen otros si los dos salieron del directorio.
        if let Some(r) = self.setting("rele_auto")? {
            let mine = split(&r);
            if mine.iter().any(|m| all.contains(m)) {
                return Ok(mine);
            }
        }
        let all: Vec<&str> = all.iter().map(String::as_str).collect();
        let mine = crate::relay::choose(&all, &self.mailbox_secret()?);
        self.set_setting("rele_auto", Some(&mine.join("|")))?;
        Ok(mine)
    }

    /// Secreto de mi buzón (el mismo en todos los relés; se crea la primera vez).
    pub fn mailbox_secret(&self) -> Result<String> {
        if let Some(s) = self.setting("buzon")? {
            return Ok(s);
        }
        let s = self.new_id()? + &self.new_id()?;
        self.set_setting("buzon", Some(&s))?;
        Ok(s)
    }

    /// Primer relé y secreto de mi buzón.
    pub fn relay(&self) -> Result<Option<(String, String)>> {
        let Some(relay) = self.relays()?.into_iter().next() else { return Ok(None) };
        Ok(Some((relay, self.mailbox_secret()?)))
    }

    /// Ruta por los relés para mi invitación: «relay=host:puerto/buzón», varias separadas por «|».
    pub fn relay_route(&self) -> Result<Option<String>> {
        let relays = self.relays()?;
        if relays.is_empty() {
            return Ok(None);
        }
        let id = crate::relay::mailbox_id(&self.mailbox_secret()?);
        Ok(Some(relays.iter().map(|r| format!("relay={r}/{id}")).collect::<Vec<_>>().join("|")))
    }

    // ---------- Buzón de cartas ----------

    pub fn set_contact_device(&self, account: &str, device: &str) -> Result<()> {
        self.db.execute("UPDATE contacts SET device = ?2 WHERE key = ?1", params![account, device])?;
        Ok(())
    }

    /// Último dispositivo del contacto con el que hablé (para cifrarle cartas).
    pub fn contact_device(&self, account: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT device FROM contacts WHERE key = ?1", [account], |r| r.get(0)).optional()?.flatten())
    }

    pub fn set_device_account(&self, device: &str, account: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO device_accounts(device, account) VALUES (?1, ?2) ON CONFLICT(device) DO UPDATE SET account = excluded.account",
            params![device, account],
        )?;
        Ok(())
    }

    /// Cuenta de un dispositivo, comprobada en un saludo anterior.
    pub fn device_account(&self, device: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT account FROM device_accounts WHERE device = ?1", [device], |r| r.get(0)).optional()?)
    }

    /// Mensajes dejados en un buzón que nadie confirmó en `days` días vuelven a pendientes.
    pub fn requeue_stale_letters(&self, days: i64) -> Result<usize> {
        Ok(self.db.execute(
            "UPDATE messages SET status = 'pendiente' WHERE outgoing = 1 AND status = 'en buzón' AND created_at < unixepoch() - ?1 * 86400",
            [days],
        )?)
    }

    // ---------- Permisos (Fase 10) ----------

    /// Lista firmada de retirados de una cuenta: (versión, datos firmados).
    pub fn revocations(&self, account: &str) -> Result<Option<(u64, String)>> {
        Ok(self
            .db
            .query_row("SELECT version, data FROM revocations WHERE account = ?1", [account], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get(1)?))
            })
            .optional()?)
    }

    pub fn set_revocations(&self, account: &str, version: u64, data: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO revocations(account, version, data) VALUES (?1, ?2, ?3)
             ON CONFLICT(account) DO UPDATE SET version = excluded.version, data = excluded.data",
            params![account, version as i64, data],
        )?;
        Ok(())
    }

    pub fn is_revoked(&self, account: &str, device: &str) -> Result<bool> {
        Ok(match self.revocations(account)? {
            Some((_, data)) => crate::access::verify(&data, account)?.revoked.iter().any(|d| d == device),
            None => false,
        })
    }

    pub fn set_blocked(&self, account: &str, blocked: bool) -> Result<()> {
        if blocked {
            self.db.execute("INSERT OR IGNORE INTO blocked(account) VALUES (?1)", [account])?;
        } else {
            self.db.execute("DELETE FROM blocked WHERE account = ?1", [account])?;
        }
        Ok(())
    }

    pub fn is_blocked(&self, account: &str) -> Result<bool> {
        Ok(self.db.query_row("SELECT 1 FROM blocked WHERE account = ?1", [account], |_| Ok(())).optional()?.is_some())
    }

    pub fn seen_device(&self, device: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO seen_devices(device) VALUES (?1) ON CONFLICT(device) DO UPDATE SET seen = unixepoch()",
            [device],
        )?;
        Ok(())
    }

    /// Dispositivos de mi cuenta vistos alguna vez: (clave, último contacto en segundos unix).
    pub fn seen_devices(&self) -> Result<Vec<(String, i64)>> {
        let mut st = self.db.prepare("SELECT device, seen FROM seen_devices ORDER BY seen DESC")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_share(&self, collection: &str, account: &str, access: Option<bool>) -> Result<()> {
        match access {
            Some(can_write) => self.db.execute(
                "INSERT INTO shares(collection, account, can_write) VALUES (?1, ?2, ?3)
                 ON CONFLICT(collection, account) DO UPDATE SET can_write = excluded.can_write",
                params![collection, account, can_write],
            )?,
            None => self.db.execute("DELETE FROM shares WHERE collection = ?1 AND account = ?2", params![collection, account])?,
        };
        Ok(())
    }

    /// Colecciones compartidas con `account`: (colección, puede cambiarla).
    pub fn shares(&self, account: &str) -> Result<Vec<(String, bool)>> {
        let mut st = self.db.prepare("SELECT collection, can_write FROM shares WHERE account = ?1 ORDER BY collection")?;
        let rows = st.query_map([account], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---------- Copias en otros dispositivos (Fase 8) ----------

    /// Dónde va una copia: la mía (`None`) o la que guardo para `owner`.
    pub fn replica_path(&self, owner: Option<&str>) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.replicas)?;
        Ok(self.replicas.join(owner.map_or("mia".to_string(), |o| format!("de-{}", &o[..16.min(o.len())]))))
    }

    pub fn set_consent(&self, owner: &str, allow: bool) -> Result<()> {
        if allow {
            self.db.execute("INSERT OR IGNORE INTO replica_consent(owner) VALUES (?1)", [owner])?;
        } else {
            self.db.execute("DELETE FROM replica_consent WHERE owner = ?1", [owner])?;
        }
        Ok(())
    }

    pub fn has_consent(&self, owner: &str) -> Result<bool> {
        Ok(self.db.query_row("SELECT 1 FROM replica_consent WHERE owner = ?1", [owner], |_| Ok(())).optional()?.is_some())
    }

    pub fn put_held(&self, owner: &str, version: u64, meta: &str, size: u64) -> Result<()> {
        self.db.execute(
            "INSERT INTO replicas_held(owner, version, meta, size) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(owner) DO UPDATE SET version = excluded.version, meta = excluded.meta,
               size = excluded.size, updated_at = unixepoch()",
            params![owner, version as i64, meta, size as i64],
        )?;
        Ok(())
    }

    /// Copia que guardo para `owner`: (versión, datos cifrados de la copia, tamaño).
    pub fn held(&self, owner: &str) -> Result<Option<(u64, String, u64)>> {
        Ok(self
            .db
            .query_row("SELECT version, meta, size FROM replicas_held WHERE owner = ?1", [owner], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get(1)?, r.get::<_, i64>(2)? as u64))
            })
            .optional()?)
    }

    /// Copias que guardo para otros como (cuenta, nombre si es contacto, tamaño).
    pub fn held_list(&self) -> Result<Vec<(String, Option<String>, u64)>> {
        let mut st = self.db.prepare(
            "SELECT h.owner, c.name, h.size FROM replicas_held h LEFT JOIN contacts c ON c.key = h.owner ORDER BY h.owner",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// El dueño dio señales (comprobó o pidió su copia): no está abandonada.
    pub fn touch_held(&self, owner: &str) -> Result<()> {
        self.db.execute("UPDATE replicas_held SET updated_at = unixepoch() WHERE owner = ?1", [owner])?;
        Ok(())
    }

    /// Copias guardadas para otros sin señales desde `before` (segundos Unix) y sin consentimiento
    /// expreso (las de amigos no caducan).
    pub fn stale_held(&self, before: i64) -> Result<Vec<String>> {
        let mut st = self.db.prepare(
            "SELECT owner FROM replicas_held WHERE updated_at < ?1 AND owner NOT IN (SELECT owner FROM replica_consent)",
        )?;
        let rows = st.query_map([before], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_held(&self, owner: &str) -> Result<()> {
        self.db.execute("DELETE FROM replicas_held WHERE owner = ?1", [owner])?;
        let path = self.replica_path(Some(owner))?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        // También las recepciones a medias («de-…<versión>.parcial»).
        if let (Some(dir), Some(stem)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) {
            for entry in std::fs::read_dir(dir)?.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("{stem}.")) && name.ends_with(".parcial") {
                    std::fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    pub fn add_holder(&self, name: &str, account: &str, addr: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO replica_holders(name, account, addr) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET addr = excluded.addr,
               status = CASE WHEN status = 'borrando' THEN 'prevista' ELSE status END",
            params![name, account, addr],
        )?;
        Ok(())
    }

    pub fn holders(&self) -> Result<Vec<Holder>> {
        let mut st = self.db.prepare("SELECT name, account, addr, version, status FROM replica_holders ORDER BY name")?;
        let rows = st.query_map([], |r| {
            Ok(Holder { name: r.get(0)?, account: r.get(1)?, addr: r.get(2)?, version: r.get::<_, i64>(3)? as u64, status: r.get(4)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_holder(&self, name: &str, version: u64, status: &str) -> Result<()> {
        self.db.execute(
            "UPDATE replica_holders SET version = ?2, status = ?3, checked_at = unixepoch() WHERE name = ?1",
            params![name, version as i64, status],
        )?;
        Ok(())
    }

    pub fn remove_holder(&self, name: &str) -> Result<()> {
        self.db.execute("DELETE FROM replica_holders WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn set_my_replica(&self, version: u64, meta: &str, size: u64) -> Result<()> {
        self.db.execute(
            "INSERT INTO my_replica(id, version, meta, size) VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET version = excluded.version, meta = excluded.meta, size = excluded.size",
            params![version as i64, meta, size as i64],
        )?;
        Ok(())
    }

    /// Mi copia más reciente: (versión, datos cifrados de la copia, tamaño).
    pub fn my_replica(&self) -> Result<Option<(u64, String, u64)>> {
        Ok(self
            .db
            .query_row("SELECT version, meta, size FROM my_replica", [], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get(1)?, r.get::<_, i64>(2)? as u64))
            })
            .optional()?)
    }

    pub fn export_messages(&self) -> Result<Vec<SnapMsg>> {
        let mut st = self.db.prepare("SELECT id, peer, outgoing, body, status, file, created_at FROM messages ORDER BY rowid")?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?, r.get::<_, String>(3)?, r.get(4)?, r.get::<_, Option<String>>(5)?, r.get(6)?))
        })?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|(id, peer, outgoing, body, status, file, created_at)| SnapMsg {
                body: self.unseal(&id, &body),
                file: self.unseal_file(&id, file),
                id,
                peer,
                outgoing,
                status,
                created_at,
            })
            .collect())
    }

    /// Recupera un mensaje desde una copia (si ya estaba, no se duplica).
    pub fn import_message(&self, m: &SnapMsg) -> Result<()> {
        let file = m.file.as_ref().map(|f| self.seal_file(&m.id, f)).transpose()?;
        self.db.execute(
            "INSERT OR IGNORE INTO messages(id, peer, outgoing, body, status, file, progress, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 100, ?7)",
            params![m.id, m.peer, m.outgoing, self.seal(&m.id, &m.body)?, m.status, file, m.created_at],
        )?;
        Ok(())
    }
}
