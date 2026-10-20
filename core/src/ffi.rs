//! Puerta de entrada para Android (Kotlin) y, más adelante, iPhone (Swift). UniFFI genera el resto.

use crate::{
    Identity, Pending, Store,
    identity::{self, Sealer, Stored},
    recovery, serve,
};
use crate::sync::{self, Policy};
use std::{
    collections::HashMap,
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Modo sin conexión: no se envía, no se sincroniza y no se aceptan conexiones; todo queda en local.
static OFFLINE: AtomicBool = AtomicBool::new(false);
static BACKGROUND: AtomicBool = AtomicBool::new(false);
/// Mi ruta pública directa si el router de casa abrió el puerto por UPnP.
static PUBLIC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// El supernodo de este teléfono, si está haciendo de relé para otros.
static SUPERNODO: std::sync::Mutex<Option<crate::supernodo::Supernodo>> = std::sync::Mutex::new(None);

#[derive(Debug, uniffi::Error)]
pub enum MeshError {
    Fallo { msg: String },
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Fallo { msg } = self;
        f.write_str(msg)
    }
}

impl std::error::Error for MeshError {}

impl From<Box<dyn std::error::Error + Send + Sync>> for MeshError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Fallo { msg: e.to_string() }
    }
}

impl From<uniffi::UnexpectedUniFFICallbackError> for MeshError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Fallo { msg: e.reason }
    }
}

fn fail(msg: impl Into<String>) -> MeshError {
    MeshError::Fallo { msg: msg.into() }
}

type R<T> = std::result::Result<T, MeshError>;

/// Protección de la clave que aporta cada plataforma (Android: Keystore).
#[uniffi::export(with_foreign)]
pub trait KeyGuard: Send + Sync {
    fn seal(&self, plain: Vec<u8>) -> R<Vec<u8>>;
    fn open(&self, sealed: Vec<u8>) -> R<Vec<u8>>;
}

/// Avisos del núcleo a la app (Fase 11): la app los usa para mostrar una notificación local.
/// Nunca llevan el contenido del mensaje.
#[uniffi::export(with_foreign)]
pub trait Events: Send + Sync {
    /// Han llegado y se han guardado `count` mensajes o archivos nuevos.
    fn received(&self, count: u32);
}

struct Guard(Arc<dyn KeyGuard>);

impl Sealer for Guard {
    fn seal(&self, plain: &[u8]) -> crate::Result<Vec<u8>> {
        Ok(self.0.seal(plain.to_vec())?)
    }
    fn open(&self, sealed: &[u8]) -> crate::Result<Vec<u8>> {
        Ok(self.0.open(sealed.to_vec())?)
    }
}

#[derive(uniffi::Record)]
pub struct Message {
    pub id: String,
    /// Nombre del contacto.
    pub peer: String,
    /// Cuenta del contacto (para `export_file`).
    pub peer_key: String,
    pub outgoing: bool,
    pub body: String,
    pub status: String,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub mime: Option<String>,
    /// Porcentaje transferido de un archivo.
    pub progress: u8,
    /// Si el archivo está en este teléfono (los recuperados de una copia no traen el archivo).
    pub file_available: bool,
}

#[derive(uniffi::Record)]
pub struct Contact {
    pub name: String,
    pub key: String,
    pub addr: Option<String>,
}

#[derive(uniffi::Record)]
pub struct Field {
    pub name: String,
    pub value: String,
}

#[derive(uniffi::Record)]
pub struct Conflict {
    pub field: String,
    /// Valores que quedaron fuera y esperan que alguien elija.
    pub values: Vec<String>,
}

#[derive(uniffi::Record)]
pub struct SyncRecord {
    pub id: String,
    pub fields: Vec<Field>,
    pub conflicts: Vec<Conflict>,
}

/// Condición de una consulta (ver `sync::Filter`).
#[derive(uniffi::Record)]
pub struct QueryFilter {
    pub field: String,
    pub op: String,
    pub value: String,
}

/// Resultado de un envío.
#[derive(uniffi::Record)]
pub struct SendResult {
    pub count: u32,
    /// `true`: el destino estaba dormido y quedó en su buzón del relé (lo recogerá al despertar).
    pub in_mailbox: bool,
}

#[derive(uniffi::Record)]
pub struct SeenDevice {
    pub key: String,
    pub fingerprint: String,
    /// Este mismo dispositivo.
    pub is_me: bool,
    pub revoked: bool,
}

#[derive(uniffi::Record)]
pub struct CopyHolder {
    pub name: String,
    /// prevista | al día | desactualizada | verificada | dañada | sin respuesta | borrando
    pub status: String,
}

#[derive(uniffi::Record)]
pub struct HeldCopy {
    /// Nombre del contacto (o principio de su cuenta).
    pub owner: String,
    pub size: u64,
}

#[derive(uniffi::Record)]
pub struct Invite {
    pub key: String,
    pub addr: String,
}

#[derive(uniffi::Enum)]
pub enum IdentityState {
    /// Nada guardado: crear cuenta, restaurar o vincular.
    Nothing,
    /// Esperando el certificado del dispositivo principal.
    Pending,
    /// Hay identidad (puede estar dañada: lo dirá `Mesh::open`).
    Ready,
}

#[uniffi::export]
pub fn identity_state(dir: String, guard: Arc<dyn KeyGuard>) -> IdentityState {
    let dir = PathBuf::from(dir);
    match identity::load_any(&dir, &Guard(guard)) {
        _ if !identity::exists(&dir) => IdentityState::Nothing,
        Ok(Stored::Pending(_)) => IdentityState::Pending,
        _ => IdentityState::Ready,
    }
}

#[derive(uniffi::Record)]
pub struct LinkRequest {
    /// Texto para el QR.
    pub text: String,
    /// Código corto que debe verse igual en los dos teléfonos.
    pub fingerprint: String,
}

/// Prepara este dispositivo para unirse a una cuenta existente (o devuelve la solicitud ya preparada).
#[uniffi::export]
pub fn link_request(dir: String, guard: Arc<dyn KeyGuard>) -> R<LinkRequest> {
    let (dir, guard) = (PathBuf::from(dir), Guard(guard));
    let p = match identity::exists(&dir) {
        false => Pending::create(&dir, &guard)?,
        true => match identity::load_any(&dir, &guard)? {
            Stored::Pending(p) => p,
            Stored::Ready(_) => return Err(fail("este dispositivo ya pertenece a una cuenta")),
        },
    };
    Ok(LinkRequest { text: crate::link_request(&p), fingerprint: crate::fingerprint(&p.public) })
}

/// Código corto de una solicitud, para que el principal lo compare antes de aprobar.
#[uniffi::export]
pub fn link_fingerprint(request: String) -> R<String> {
    Ok(crate::fingerprint(&crate::parse_link_request(&request)?))
}

#[uniffi::export]
pub fn parse_invite(text: String) -> R<Invite> {
    let (key, addr) = crate::parse_invite(&text)?;
    Ok(Invite { key, addr })
}

/// Nombre con el que aparece el guardián universal entre los contactos.
const GUARDIAN: &str = "guardián";

/// Guardián universal: el primero del directorio (o de la lista fija, en un teléfono nuevo sin
/// directorio, después de intentar descargarlo de los relés universales).
/// Guía de nombres: la elegida con `use_name_guide` o la universal.
fn name_guide(store: &Store) -> R<String> {
    store.setting("guia_elegida")?.or_else(|| crate::nombres::UNIVERSALES.first().map(|g| g.to_string())).ok_or_else(|| fail("no hay ninguna guía de nombres configurada"))
}

fn universal_guardian(store: Option<&Store>) -> R<(String, String)> {
    let list = match store {
        Some(s) => crate::directorio::guardianes(s),
        None => {
            let fijos: Vec<String> = crate::relay::UNIVERSALES.iter().map(|r| r.to_string()).collect();
            crate::directorio::descargar(&fijos).map(|(d, _)| d.guardianes).filter(|g| !g.is_empty())
                .unwrap_or_else(|| crate::guardian::UNIVERSALES.iter().map(|g| g.to_string()).collect())
        }
    };
    let inv = list.first().ok_or_else(|| fail("no hay ningún guardián configurado"))?;
    Ok(crate::parse_invite(inv)?)
}

/// Un dispositivo MobileMesh. Cada llamada abre su propia conexión a SQLite, así nada se bloquea entre hilos.
#[derive(uniffi::Object)]
pub struct Mesh {
    dir: PathBuf,
    me: Identity,
}

impl Mesh {
    fn holder(&self, store: &Store, name: &str) -> R<crate::store::Holder> {
        if let Some((_, key, addr)) = store.contacts()?.into_iter().find(|c| c.0 == name) {
            store.add_holder(name, &key, &addr.ok_or_else(|| fail(format!("no sé la dirección de «{name}»")))?)?;
        } else if let Some((_, addr)) = store.my_devices()?.into_iter().find(|d| d.0 == name) {
            store.add_holder(name, &self.me.id(), &addr)?;
        } else {
            return Err(fail(format!("«{name}» no es un contacto ni un dispositivo tuyo")));
        }
        Ok(store.holders()?.into_iter().find(|h| h.name == name).expect("recién añadido"))
    }

    fn ready(dir: PathBuf, me: Identity) -> R<Arc<Self>> {
        Store::open(&dir, me.db_key())?;
        Ok(Arc::new(Mesh { dir, me }))
    }
}

#[uniffi::export]
impl Mesh {
    /// Crea una identidad nueva. Falla si ya hay una.
    #[uniffi::constructor]
    pub fn create(dir: String, guard: Arc<dyn KeyGuard>) -> R<Arc<Self>> {
        let dir = PathBuf::from(dir);
        let me = Identity::create(&dir, &Guard(guard))?;
        Self::ready(dir, me)
    }

    #[uniffi::constructor]
    pub fn open(dir: String, guard: Arc<dyn KeyGuard>) -> R<Arc<Self>> {
        let dir = PathBuf::from(dir);
        let me = Identity::load(&dir, &Guard(guard))?;
        Self::ready(dir, me)
    }

    /// Termina la vinculación con el certificado que muestra el dispositivo principal.
    #[uniffi::constructor]
    pub fn complete_link(dir: String, guard: Arc<dyn KeyGuard>, answer: String) -> R<Arc<Self>> {
        let (dir, guard) = (PathBuf::from(dir), Guard(guard));
        let Stored::Pending(p) = identity::load_any(&dir, &guard)? else {
            return Err(fail("este dispositivo no está esperando una vinculación"));
        };
        let me = crate::complete_link(p, &dir, &guard, &answer)?;
        Self::ready(dir, me)
    }

    /// Recupera la cuenta desde la copia. Solo si no hay una identidad válida en este dispositivo.
    #[uniffi::constructor]
    pub fn restore(dir: String, guard: Arc<dyn KeyGuard>, code: String, pass: String) -> R<Arc<Self>> {
        let (dir, guard) = (PathBuf::from(dir), Guard(guard));
        if Identity::load(&dir, &guard).is_ok() {
            return Err(fail("este dispositivo ya tiene una identidad válida; no se reemplaza"));
        }
        let me = recovery::import(&code, &pass)?;
        me.save(&dir, &guard)?;
        Self::ready(dir, me)
    }

    /// Recupera la cuenta con el código en papel (sin contraseña). Después, `use_guardian` +
    /// `restore_copy(GUARDIAN)` traen los datos.
    #[uniffi::constructor]
    pub fn restore_paper(dir: String, guard: Arc<dyn KeyGuard>, code: String) -> R<Arc<Self>> {
        let (dir, guard) = (PathBuf::from(dir), Guard(guard));
        if Identity::load(&dir, &guard).is_ok() {
            return Err(fail("este dispositivo ya tiene una identidad válida; no se reemplaza"));
        }
        let me = recovery::from_paper(&code)?;
        me.save(&dir, &guard)?;
        Self::ready(dir, me)
    }

    /// Recupera la cuenta desde un guardián con usuario y contraseña (teléfono perdido). Después,
    /// `use_guardian` + `restore_copy(GUARDIAN)` traen los datos.
    #[uniffi::constructor]
    pub fn recover_from_guardian(dir: String, guard: Arc<dyn KeyGuard>, user: String, pass: String) -> R<Arc<Self>> {
        let (dir, guard) = (PathBuf::from(dir), Guard(guard));
        if Identity::load(&dir, &guard).is_ok() {
            return Err(fail("este dispositivo ya tiene una identidad válida; no se reemplaza"));
        }
        let (key, route) = universal_guardian(None)?;
        let me = crate::guardian::find_account(&dir.with_extension("paso"), &route, &key, &user, &pass)?;
        me.save(&dir, &guard)?;
        Self::ready(dir, me)
    }

    pub fn id(&self) -> String {
        self.me.id()
    }

    /// Añade el guardián universal como contacto y como guardián de mi copia (desde ahora
    /// `backup_now` también le envía la copia). Devuelve su nombre.
    pub fn use_guardian(&self) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let (key, route) = universal_guardian(Some(&store))?;
        if store.contact_key(GUARDIAN)?.is_none() {
            store.add_contact(GUARDIAN, &key, Some(&route))?;
        }
        self.holder(&store, GUARDIAN)?;
        Ok(GUARDIAN.into())
    }

    /// Guarda en el guardián mi código de recuperación, para recuperarlo con usuario + contraseña.
    pub fn save_account_in_guardian(&self, user: String, pass: String) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let (key, route) = universal_guardian(Some(&store))?;
        Ok(crate::guardian::save_account(&store, &self.me, &route, &key, &user, &pass)?)
    }

    // ---------- Guía de nombres ----------

    /// Espacio de nombres de esta app en la guía (el SDK pone el paquete de la app): así los
    /// nombres de una app no chocan con los de otras que usan la misma guía.
    pub fn set_app(&self, app: String) -> R<()> {
        let app = crate::nombres::normalize_app(&app)?;
        Ok(Store::open(&self.dir, self.me.db_key())?.set_setting("app", Some(&app).filter(|a| !a.is_empty()).map(|a| a.as_str()))?)
    }

    /// Elige la guía de nombres (su invitación), o vuelve a la universal con `None`.
    pub fn use_name_guide(&self, invitation: Option<String>) -> R<()> {
        if let Some(i) = &invitation {
            crate::parse_invite(i)?;
        }
        Ok(Store::open(&self.dir, self.me.db_key())?.set_setting("guia_elegida", invitation.as_deref())?)
    }

    /// Registra mi nombre de usuario con mi invitación (llamarlo de nuevo actualiza la invitación).
    pub fn register_name(&self, name: String, invitation: String) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        Ok(crate::nombres::register(&store, &self.me, &name_guide(&store)?, &name, &invitation)?)
    }

    /// Invitación de un nombre de usuario, si existe.
    pub fn find_name(&self, name: String) -> R<Option<String>> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        Ok(crate::nombres::find(&store, &self.me, &name_guide(&store)?, &name)?)
    }

    /// Añade a un usuario por su nombre y le avisa (su app me añadirá sola al ver la solicitud).
    pub fn add_by_name(&self, name: String) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let guia = name_guide(&store)?;
        let name = crate::nombres::normalize(&name)?;
        let inv = crate::nombres::find(&store, &self.me, &guia, &name)?.ok_or_else(|| fail(format!("no existe el usuario «{name}»")))?;
        let (key, route) = crate::parse_invite(&inv)?;
        store.add_contact(&name, &key, Some(&route))?;
        Ok(crate::nombres::request(&store, &self.me, &guia, &name)?)
    }

    /// Añade como contactos a quienes pidieron hablar conmigo. Devuelve sus nombres.
    pub fn accept_requests(&self) -> R<Vec<String>> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let mut nuevos = vec![];
        for (name, inv) in crate::nombres::requests(&store, &self.me, &name_guide(&store)?)? {
            let Ok((key, route)) = crate::parse_invite(&inv) else { continue };
            // Si el nombre ya es de otro contacto (otra clave), se deja como está.
            if store.contact_key(&name)?.is_none_or(|k| k == key) && store.add_contact(&name, &key, Some(&route)).is_ok() {
                nuevos.push(name);
            }
        }
        Ok(nuevos)
    }

    /// Código corto de este dispositivo.
    pub fn fingerprint(&self) -> String {
        crate::fingerprint(&self.me.public)
    }

    /// Si es el dispositivo principal (puede vincular otros y crear la copia).
    pub fn is_primary(&self) -> bool {
        self.me.account_secret.is_some()
    }

    /// Aprueba la solicitud de otro dispositivo; devuelve el certificado para mostrarle en QR.
    pub fn approve_link(&self, request: String) -> R<String> {
        Ok(crate::approve_link(&self.me, &request)?)
    }

    /// Copia de recuperación cifrada con `pass`, para guardar fuera del teléfono.
    pub fn backup(&self, pass: String) -> R<String> {
        Ok(recovery::export(&self.me, &pass)?)
    }

    /// Código en papel de la cuenta (56 letras): recupera la cuenta sin contraseña.
    pub fn paper_code(&self) -> R<String> {
        Ok(recovery::paper(&self.me)?)
    }

    /// Invitación (para el QR) con la que otro dispositivo nos añade: dirección directa y, si hay
    /// relé, también la ruta por el relé (se usa solo si la directa no funciona).
    pub fn invite(&self, addr: String) -> R<String> {
        let relay = Store::open(&self.dir, self.me.db_key())?.relay_route()?;
        let route = relay.map_or(addr.clone(), |r| format!("{addr}|{r}"));
        Ok(crate::invite(&self.me, &route))
    }

    /// Usa sus propios relés «host:puerto» (varios separados por «|») en vez de los universales, o
    /// ninguno con `None`. Se aplica al reiniciar la app.
    // ponytail: el relé se lee al arrancar; cambiarlo en caliente si hace falta.
    pub fn set_relay(&self, relay: Option<String>) -> R<()> {
        let relay = relay.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
        if let Some(r) = &relay {
            crate::transport::check_route(r)?;
        }
        let store = Store::open(&self.dir, self.me.db_key())?;
        store.set_setting("rele", Some(relay.as_deref().unwrap_or(crate::relay::SIN_RELE)))?;
        Ok(())
    }

    /// Relé configurado y buzón (para mostrar).
    /// Avisos opcionales (p. ej. Firebase): guarda en mi relé la ficha con la que despertarme cuando
    /// me dejen una carta, o la quita con `None`. El aviso no lleva contenido.
    pub fn set_push_token(&self, token: Option<String>) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let (relay, secret) = store.relay()?.ok_or_else(|| fail("los avisos necesitan un relé: configura uno primero"))?;
        Ok(crate::relay::set_push_token(&relay, &secret, token.as_deref())?)
    }

    /// Mi ruta pública directa («ip:puerto») si el router abrió el puerto por UPnP.
    pub fn public_route(&self) -> Option<String> {
        PUBLIC.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn relay_info(&self) -> R<Option<String>> {
        Ok(Store::open(&self.dir, self.me.db_key())?.relay_route()?)
    }

    /// Añade un contacto. Devuelve `true` si la invitación era de otro dispositivo de mi cuenta.
    pub fn add_contact(&self, name: String, key: String, addr: Option<String>) -> R<bool> {
        let name = name.trim();
        if name.is_empty() {
            return Err(fail("escribe un nombre para el contacto"));
        }
        let store = Store::open(&self.dir, self.me.db_key())?;
        Ok(crate::add_contact_or_device(&store, &self.me, name, key.trim(), addr.as_deref().map(str::trim))?)
    }

    /// Otros dispositivos de mi cuenta con los que sincronizo (nombre y dirección).
    pub fn my_devices(&self) -> R<Vec<Contact>> {
        let rows = Store::open(&self.dir, self.me.db_key())?.my_devices()?;
        Ok(rows.into_iter().map(|(name, addr)| Contact { name, key: self.me.id(), addr: Some(addr) }).collect())
    }

    // ---------- Datos sincronizados ----------

    /// Guarda un campo (en local al momento; se sincroniza después). `strict`: los cambios hechos a
    /// la vez en dos dispositivos no se mezclan, quedan en conflicto. `value` vacío = borrar.
    pub fn put(&self, collection: String, record: String, field: String, value: Option<String>, strict: bool) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let policy = if strict { Policy::Strict } else { Policy::Lww };
        let device = identity::to_hex(&self.me.public);
        Ok(sync::put_as(&store, &device, &self.me.id(), policy, &collection, &record, &field, value.as_deref())?)
    }

    pub fn records(&self, collection: String) -> R<Vec<SyncRecord>> {
        let rows = sync::records(&Store::open(&self.dir, self.me.db_key())?, &collection)?;
        Ok(rows
            .into_iter()
            .map(|r| SyncRecord {
                id: r.id,
                fields: r.fields.into_iter().map(|(name, value)| Field { name, value }).collect(),
                conflicts: r.conflicts.into_iter().map(|(field, values)| Conflict { field, values }).collect(),
            })
            .collect())
    }

    /// Consulta: documentos que cumplen todas las condiciones, ordenados y limitados.
    pub fn query(&self, collection: String, filters: Vec<QueryFilter>, order_by: Option<String>, descending: bool, limit: Option<u32>) -> R<Vec<SyncRecord>> {
        let filters: Vec<sync::Filter> = filters.into_iter().map(|f| sync::Filter { field: f.field, op: f.op, value: f.value }).collect();
        let store = Store::open(&self.dir, self.me.db_key())?;
        let rows = sync::query(&store, &collection, &filters, order_by.as_deref().map(|f| (f, descending)), limit.map(|n| n as usize))?;
        Ok(rows
            .into_iter()
            .map(|r| SyncRecord {
                id: r.id,
                fields: r.fields.into_iter().map(|(name, value)| Field { name, value }).collect(),
                conflicts: r.conflicts.into_iter().map(|(field, values)| Conflict { field, values }).collect(),
            })
            .collect())
    }

    /// Funciones de servidor: llama a la función `function` del ayudante `name` (un contacto) con
    /// `data` (JSON). `my_route`: mi ruta para la respuesta. Devuelve el identificador de la llamada.
    pub fn call_helper(&self, name: String, function: String, data: String, my_route: String) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let c = store.contacts()?.into_iter().find(|c| c.0 == name).ok_or_else(|| fail(format!("no existe el ayudante «{name}»")))?;
        let addr = c.2.ok_or_else(|| fail(format!("no sé la dirección de «{name}»")))?;
        Ok(crate::funciones::call(&store, &self.me, &c.1, &addr, &function, &data, &my_route)?)
    }

    /// Respuesta de una llamada, si ya llegó. Si el ayudante respondió con error, falla con ese error.
    pub fn helper_reply(&self, name: String, call_id: String) -> R<Option<String>> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let key = store.contact_key(&name)?.ok_or_else(|| fail(format!("no existe el ayudante «{name}»")))?;
        match crate::funciones::reply(&store, &key, &call_id)? {
            None => Ok(None),
            Some(Ok(v)) => Ok(Some(v)),
            Some(Err(e)) => Err(fail(e)),
        }
    }

    /// Cambios propios que algún dispositivo propio aún no ha confirmado.
    pub fn unsynced(&self) -> R<u64> {
        Ok(sync::unsynced(&Store::open(&self.dir, self.me.db_key())?, &identity::to_hex(&self.me.public))?)
    }

    /// Sincroniza ya con todos mis dispositivos. Devuelve un resumen para mostrar.
    pub fn sync_now(&self) -> R<String> {
        if OFFLINE.load(Ordering::SeqCst) {
            return Ok("Modo sin conexión: los cambios quedan guardados en este teléfono".into());
        }
        let store = Store::open(&self.dir, self.me.db_key())?;
        let devices = store.my_devices()?;
        if devices.is_empty() {
            return Ok("No hay otros dispositivos de tu cuenta: añade la invitación de tu otro teléfono".into());
        }
        let lines: Vec<String> = devices
            .iter()
            .map(|(name, addr)| match crate::sync_with(&store, &self.me, addr.as_str()) {
                Ok((got, sent)) => format!("{name}: {got} cambio(s) recibido(s), {sent} enviado(s)"),
                Err(e) => format!("{name}: no disponible ({e})"),
            })
            .collect();
        Ok(lines.join("\n"))
    }

    // ---------- Copias en otros dispositivos ----------

    /// Elige a `name` (contacto o dispositivo propio) para guardar mi copia cifrada.
    pub fn add_copy_holder(&self, name: String) -> R<()> {
        self.holder(&Store::open(&self.dir, self.me.db_key())?, &name).map(drop)
    }

    pub fn copy_holders(&self) -> R<Vec<CopyHolder>> {
        let rows = Store::open(&self.dir, self.me.db_key())?.holders()?;
        Ok(rows.into_iter().map(|h| CopyHolder { name: h.name, status: h.status }).collect())
    }

    /// Hace una copia nueva y la envía a todos los que la guardan. Devuelve un resumen.
    pub fn backup_now(&self) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let holders = store.holders()?;
        if holders.is_empty() {
            return Err(fail("elige primero quién guardará tu copia"));
        }
        crate::replica::snapshot(&store, &self.me)?;
        if OFFLINE.load(Ordering::SeqCst) {
            return Ok("Copia hecha; se enviará al volver la conexión".into());
        }
        let lines: Vec<String> = store
            .holders()?
            .iter()
            .filter(|h| h.status != "borrando")
            .map(|h| match crate::replica::push(&store, &self.me, h) {
                Ok(()) => format!("{}: copia guardada", h.name),
                Err(e) => {
                    let _ = store.set_holder(&h.name, h.version, "sin respuesta");
                    format!("{}: no disponible ({e})", h.name)
                }
            })
            .collect();
        Ok(lines.join("\n"))
    }

    /// Comprueba, sin descargarla, que cada uno tiene mi copia entera y al día.
    pub fn check_copies(&self) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let lines: Vec<String> = store
            .holders()?
            .iter()
            .filter(|h| h.status != "borrando")
            .map(|h| match crate::replica::check(&store, &self.me, h) {
                Ok(status) => format!("{}: {status}", h.name),
                Err(e) => format!("{}: no disponible ({e})", h.name),
            })
            .collect();
        Ok(lines.join("\n"))
    }

    /// Pide a `name` que borre mi copia (si no contesta, se reintenta en segundo plano).
    pub fn remove_copy_holder(&self, name: String) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let h = store.holders()?.into_iter().find(|h| h.name == name).ok_or_else(|| fail("no guarda tu copia"))?;
        Ok(match crate::replica::delete(&store, &self.me, &h) {
            Ok(()) => format!("{name} borró tu copia"),
            Err(_) => format!("{name} no está conectado: se le pedirá que la borre cuando vuelva"),
        })
    }

    /// Recupera mis datos desde la copia que guarda `name` (tras recuperar la cuenta en un teléfono nuevo).
    pub fn restore_copy(&self, name: String) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let h = self.holder(&store, &name)?;
        let r = crate::replica::restore(&store, &self.me, &h)?;
        Ok(format!("Recuperado de {name}: {} contacto(s), {} mensaje(s), {} dato(s)", r.contacts, r.messages, r.changes))
    }

    /// Acepto (o dejo de aceptar) guardar la copia cifrada del contacto `name`. Al dejarlo, se borra.
    pub fn allow_copies(&self, name: String, allow: bool) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let key = store.contact_key(&name)?.ok_or_else(|| fail(format!("no existe el contacto «{name}»")))?;
        store.set_consent(&key, allow)?;
        if !allow {
            store.delete_held(&key)?;
        }
        Ok(())
    }

    pub fn copies_allowed(&self, name: String) -> R<bool> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        Ok(match store.contact_key(&name)? {
            Some(key) => store.has_consent(&key)?,
            None => false,
        })
    }

    /// Copias que guardo para otros (no puedo leerlas).
    pub fn held_copies(&self) -> R<Vec<HeldCopy>> {
        let rows = Store::open(&self.dir, self.me.db_key())?.held_list()?;
        Ok(rows.into_iter().map(|(owner, name, size)| HeldCopy { owner: name.unwrap_or_else(|| owner[..12].into()), size }).collect())
    }

    // ---------- Permisos (Fase 10) ----------

    /// Dispositivos de mi cuenta conocidos (este y los que han conectado), para poder retirarlos.
    pub fn account_devices(&self) -> R<Vec<SeenDevice>> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let mine = identity::to_hex(&self.me.public);
        let mut keys = vec![mine.clone()];
        keys.extend(store.seen_devices()?.into_iter().map(|d| d.0).filter(|d| *d != mine));
        keys.iter()
            .map(|k| {
                Ok(SeenDevice {
                    fingerprint: crate::fingerprint(&identity::from_hex(k)?),
                    is_me: *k == mine,
                    revoked: store.is_revoked(&self.me.id(), k)?,
                    key: k.clone(),
                })
            })
            .collect()
    }

    /// Retira un dispositivo de la cuenta y avisa a todos los contactos y dispositivos que estén
    /// conectados (el aviso viaja al empezar cualquier conexión; los demás se enterarán después).
    pub fn revoke_device(&self, key: String) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        crate::access::revoke(&store, &self.me, &key)?;
        let mut targets: Vec<(String, String, Vec<u8>)> = store
            .contacts()?
            .into_iter()
            .filter_map(|(name, key, addr)| Some((name, addr?, identity::from_hex(&key).ok()?)))
            .collect();
        targets.extend(store.my_devices()?.into_iter().map(|(name, addr)| (name, addr, self.me.account.to_vec())));
        let told = targets.iter().filter(|(_, addr, acc)| crate::connect(&store, &self.me, addr, acc).is_ok()).count();
        Ok(format!("Dispositivo retirado. Avisados ahora: {told} de {}; los demás se enterarán al conectar.", targets.len()))
    }

    pub fn set_blocked(&self, name: String, blocked: bool) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let key = store.contact_key(&name)?.ok_or_else(|| fail(format!("no existe el contacto «{name}»")))?;
        Ok(store.set_blocked(&key, blocked)?)
    }

    pub fn is_blocked(&self, name: String) -> R<bool> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        Ok(match store.contact_key(&name)? {
            Some(key) => store.is_blocked(&key)?,
            None => false,
        })
    }

    /// Nombre de la lista compartida de una cuenta (la mía si `name` es `None`).
    pub fn list_name(&self, name: Option<String>) -> R<String> {
        let account = match name {
            None => self.me.id(),
            Some(n) => Store::open(&self.dir, self.me.db_key())?.contact_key(&n)?.ok_or_else(|| fail(format!("no existe el contacto «{n}»")))?,
        };
        Ok(format!("lista-{}", &account[..8]))
    }

    /// Comparte (o deja de compartir, con `None`) una colección con un contacto: `Some(false)` solo
    /// leer, `Some(true)` leer y cambiar. Para recibir la de un contacto, se comparte con escritura.
    pub fn share(&self, collection: String, name: String, can_write: Option<bool>) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let key = store.contact_key(&name)?.ok_or_else(|| fail(format!("no existe el contacto «{name}»")))?;
        Ok(store.set_share(&collection, &key, can_write)?)
    }

    /// Colecciones compartidas con un contacto, como «colección (solo leer | leer y cambiar)».
    pub fn shares(&self, name: String) -> R<Vec<String>> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let Some(key) = store.contact_key(&name)? else { return Ok(vec![]) };
        Ok(store.shares(&key)?.into_iter().map(|(c, w)| format!("{c} ({})", if w { "leer y cambiar" } else { "solo leer" })).collect())
    }

    /// Sincroniza con un contacto lo que compartimos.
    pub fn sync_with_contact(&self, name: String) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let (_, key, addr) = store.contacts()?.into_iter().find(|c| c.0 == name).ok_or_else(|| fail(format!("no existe el contacto «{name}»")))?;
        let addr = addr.ok_or_else(|| fail("no sé su dirección"))?;
        let (got, sent, rejected) = crate::sync_shared(&store, &self.me, &addr, &key)?;
        Ok(format!("{name}: {got} cambio(s) recibido(s), {sent} enviado(s), {rejected} rechazado(s) por falta de permiso"))
    }

    /// Resumen del estado para depurar una app (sin datos privados).
    pub fn diagnostics(&self) -> R<String> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let contacts = store.contacts()?;
        let pending: usize = contacts.iter().map(|c| store.pending(&c.1).map(|p| p.len()).unwrap_or(0)).sum();
        let lines = [
            format!("Cuenta: {}…", &self.me.id()[..16]),
            format!("Este dispositivo: {} ({})", crate::fingerprint(&self.me.public), if self.me.account_secret.is_some() { "principal" } else { "vinculado" }),
            format!("Relé: {}", store.relay_route()?.unwrap_or_else(|| "ninguno".into())),
            format!("Sin conexión (modo manual): {}", if OFFLINE.load(Ordering::SeqCst) { "sí" } else { "no" }),
            format!("Contactos: {} (bloqueados: {})", contacts.len(), contacts.iter().filter(|c| store.is_blocked(&c.1).unwrap_or(false)).count()),
            format!("Otros dispositivos de la cuenta: {}", store.my_devices()?.len()),
            format!("Mensajes pendientes de entregar: {pending}"),
            format!("Cambios sin confirmar por mis dispositivos: {}", sync::unsynced(&store, &identity::to_hex(&self.me.public))?),
            format!("Guardan mi copia: {}", store.holders()?.len()),
            format!("Guardo copias de otros: {}", store.held_list()?.len()),
        ];
        Ok(lines.join("\n"))
    }

    /// Empieza a hacer de relé ligero para otros en `port` (la app decide cuándo: wifi y batería).
    /// Abre el puerto en el router (UPnP) y se apunta en el director cada minuto. Sin UPnP no se
    /// le puede llegar desde fuera: el director no lo dará a nadie. `true` si ya estaba o arrancó.
    pub fn start_supernode(&self, port: u16) -> R<bool> {
        let mut g = SUPERNODO.lock().unwrap_or_else(|e| e.into_inner());
        if g.as_ref().is_some_and(|s| s.activo()) {
            return Ok(true);
        }
        let l = TcpListener::bind(("::", port)).or_else(|_| TcpListener::bind(("0.0.0.0", port))).map_err(|e| fail(e.to_string()))?;
        let nodo = crate::supernodo::Supernodo::arrancar(l)?;
        let director = crate::directorio::reles(&Store::open(&self.dir, self.me.db_key())?).into_iter().next().ok_or_else(|| fail("no hay relé director"))?;
        let vivo = nodo.clone();
        thread::spawn(move || {
            let mut publico = None;
            let mut vuelta = 0u32;
            while vivo.activo() {
                // El puerto del router caduca a la hora: se renueva cada 30 min.
                if publico.is_none() || vuelta.is_multiple_of(30) {
                    publico = crate::upnp::open(port, 3600).ok();
                }
                if let Some(p) = publico {
                    crate::supernodo::apuntarse(&director, p);
                }
                vuelta += 1;
                thread::sleep(Duration::from_secs(60));
            }
        });
        *g = Some(nodo);
        Ok(true)
    }

    /// Deja de hacer de relé para otros (las llamadas en curso terminan solas).
    pub fn stop_supernode(&self) {
        if let Some(n) = SUPERNODO.lock().unwrap_or_else(|e| e.into_inner()).take() {
            n.parar();
        }
    }

    pub fn is_supernode(&self) -> bool {
        SUPERNODO.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.activo())
    }

    pub fn set_offline(&self, on: bool) {
        OFFLINE.store(on, Ordering::SeqCst);
    }

    pub fn is_offline(&self) -> bool {
        OFFLINE.load(Ordering::SeqCst)
    }

    /// Cada `seconds`, reintenta los envíos pendientes y sincroniza con mis dispositivos. Si un
    /// destino no responde, espera cada vez el doble (hasta 5 minutos) antes de volver a probar.
    pub fn start_background(&self, seconds: u32, events: Option<Arc<dyn Events>>) {
        if BACKGROUND.swap(true, Ordering::SeqCst) {
            return;
        }
        let (dir, me) = (self.dir.clone(), self.me.clone());
        let mut last_collect = Instant::now() - Duration::from_secs(3600);
        let mut last_check = Instant::now();
        let every = Duration::from_secs(seconds.max(1) as u64);
        thread::spawn(move || {
            let mut wait: HashMap<String, (Instant, u32)> = HashMap::new(); // dirección → (no antes de, fallos)
            // Sincronizar solo si cambié algo desde la última vez, o cada RESYNC por si acaso: una
            // sincronización vacía gasta ~3,5 KB y cada 15 s serían ~20 MB al día (medido, Fase 13).
            // Lo que cambia el otro llega igual de rápido: es su teléfono el que sincroniza al cambiar.
            const RESYNC: Duration = Duration::from_secs(600);
            let device = identity::to_hex(&me.public);
            let mut synced: HashMap<String, (Instant, u64)> = HashMap::new(); // dirección → (cuándo, mis cambios)
            loop {
                thread::sleep(every);
                if OFFLINE.load(Ordering::SeqCst) {
                    continue;
                }
                let Ok(store) = Store::open(&dir, me.db_key()) else { continue };
                // Cartas que me dejaron mientras dormía (una vez por minuto).
                if last_collect.elapsed() > Duration::from_secs(60) {
                    last_collect = Instant::now();
                    if let (Ok(n @ 1..), Some(e)) = (crate::collect_letters(&store, &me), &events) {
                        e.received(n as u32);
                    }
                }
                let contacts = store.contacts().unwrap_or_default();
                let devices = store.my_devices().unwrap_or_default();
                let jobs = contacts
                    .into_iter()
                    .filter_map(|(_, key, addr)| Some((Some(key), addr?)))
                    .chain(devices.into_iter().map(|(_, addr)| (None, addr)));
                // Una vez al día, comprobar mis copias: detecta las dañadas y avisa a los guardianes
                // de que siguen vivas (si no, las borran a los 90 días).
                if last_check.elapsed() > Duration::from_secs(86_400) {
                    last_check = Instant::now();
                    for h in store.holders().unwrap_or_default().iter().filter(|h| h.status != "borrando") {
                        let _ = crate::replica::check(&store, &me, h); // ya apunta el estado
                    }
                }
                // Copias: enviar las que faltan o están viejas y reintentar los borrados pendientes.
                for h in store.holders().unwrap_or_default() {
                    if wait.get(&h.addr).is_some_and(|(at, _)| Instant::now() < *at) {
                        continue;
                    }
                    let ok = match h.status.as_str() {
                        "borrando" => crate::replica::delete(&store, &me, &h).is_ok(),
                        "prevista" | "desactualizada" | "sin respuesta" if store.my_replica().ok().flatten().is_some() => {
                            crate::replica::push(&store, &me, &h).is_ok()
                        }
                        _ => true,
                    };
                    if !ok {
                        let fails = wait.get(&h.addr).map_or(0, |w| w.1) + 1;
                        let pause = (every * 2u32.saturating_pow(fails)).min(Duration::from_secs(300));
                        wait.insert(h.addr.clone(), (Instant::now() + pause, fails));
                    }
                }
                let mine = store.max_seq(&device).unwrap_or(0);
                let due = |synced: &HashMap<String, (Instant, u64)>, addr: &str| {
                    synced.get(addr).is_none_or(|(t, m)| *m != mine || t.elapsed() > RESYNC)
                };
                // Colecciones compartidas: se sincronizan solas con cada contacto que las comparte.
                for (_, key, addr) in store.contacts().unwrap_or_default() {
                    let Some(addr) = addr else { continue };
                    if store.shares(&key).map(|s| s.is_empty()).unwrap_or(true)
                        || wait.get(&addr).is_some_and(|(at, _)| Instant::now() < *at)
                        || !due(&synced, &addr)
                    {
                        continue;
                    }
                    if crate::sync_shared(&store, &me, &addr, &key).is_ok() {
                        synced.insert(addr, (Instant::now(), mine));
                    }
                }
                for (key, addr) in jobs {
                    if wait.get(&addr).is_some_and(|(at, _)| Instant::now() < *at) {
                        continue;
                    }
                    let ok = match &key {
                        // Si no contesta (dormido), lo deja en su buzón del relé.
                        Some(key) => crate::send_or_deposit(&store, &me, key, addr.as_str()).is_ok(),
                        None if !due(&synced, &addr) => true,
                        None => {
                            let ok = crate::sync_with(&store, &me, addr.as_str()).is_ok();
                            if ok {
                                synced.insert(addr.clone(), (Instant::now(), mine));
                            }
                            ok
                        }
                    };
                    if ok {
                        wait.remove(&addr);
                    } else {
                        let fails = wait.get(&addr).map_or(0, |w| w.1) + 1;
                        let pause = (every * 2u32.saturating_pow(fails)).min(Duration::from_secs(300));
                        wait.insert(addr, (Instant::now() + pause, fails));
                    }
                }
            }
        });
    }

    pub fn contacts(&self) -> R<Vec<Contact>> {
        let rows = Store::open(&self.dir, self.me.db_key())?.contacts()?;
        Ok(rows.into_iter().map(|(name, key, addr)| Contact { name, key, addr }).collect())
    }

    pub fn messages(&self) -> R<Vec<Message>> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let rows = store.list()?;
        Ok(rows
            .into_iter()
            .filter(|r| !crate::funciones::is_control(&r.body)) // llamadas a ayudantes y sus respuestas
            .map(|r| Message {
                file_available: r.file.is_some() && store.blob(&r.peer_key, r.outgoing, &r.id).is_ok_and(|p| p.exists()),
                file_name: r.file.as_ref().map(|f| f.name.clone()),
                file_size: r.file.as_ref().map(|f| f.size),
                mime: r.file.as_ref().map(|f| f.mime.clone()),
                id: r.id,
                peer: r.peer,
                peer_key: r.peer_key,
                outgoing: r.outgoing,
                body: r.body,
                status: r.status,
                progress: r.progress,
            })
            .collect())
    }

    /// Guarda el texto (vacío = solo reintentar) y entrega todo lo pendiente para ese contacto.
    /// Si falla, los mensajes quedan pendientes. Bloquea: llamar fuera del hilo de la pantalla.
    pub fn send(&self, name: String, text: String) -> R<SendResult> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let c = store.contacts()?.into_iter().find(|c| c.0 == name).ok_or_else(|| fail(format!("no existe el contacto «{name}»")))?;
        let addr = c.2.ok_or_else(|| fail(format!("no sé la dirección de «{name}»; escanea su invitación")))?;
        if !text.is_empty() {
            store.queue(&c.1, &text)?;
        }
        if OFFLINE.load(Ordering::SeqCst) {
            return Err(fail("modo sin conexión: guardado en este teléfono, se enviará al volver"));
        }
        Ok(match crate::send_or_deposit(&store, &self.me, &c.1, addr.as_str())? {
            crate::Sent::Delivered(n) => SendResult { count: n as u32, in_mailbox: false },
            crate::Sent::Deposited(n) => SendResult { count: n as u32, in_mailbox: true },
        })
    }

    /// Para el despertador del sistema (avisos en frío): recoge las cartas del buzón, guarda los
    /// mensajes y reintenta lo pendiente. Devuelve cuántos mensajes nuevos llegaron.
    pub fn check_mailbox(&self) -> R<u32> {
        if OFFLINE.load(Ordering::SeqCst) {
            return Ok(0);
        }
        let store = Store::open(&self.dir, self.me.db_key())?;
        let new = crate::collect_letters(&store, &self.me)?;
        for (_, key, addr) in store.contacts()? {
            if let Some(addr) = addr
                && !store.pending(&key)?.is_empty()
            {
                let _ = crate::send_or_deposit(&store, &self.me, &key, &addr);
            }
        }
        Ok(new as u32)
    }

    /// Cifra el archivo `path` (se puede borrar después: se guarda una copia cifrada) y lo envía
    /// junto con lo pendiente. Bloquea: llamar fuera del hilo de la pantalla.
    pub fn send_file(&self, name: String, path: String, file_name: String, mime: String) -> R<SendResult> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        let key = store.contact_key(&name)?.ok_or_else(|| fail(format!("no existe el contacto «{name}»")))?;
        crate::queue_file(&store, &key, std::path::Path::new(&path), &file_name, &mime)?;
        drop(store);
        self.send(name, String::new())
    }

    /// Descifra el archivo de un mensaje en `dst`, comprobando su huella.
    pub fn export_file(&self, peer_key: String, outgoing: bool, id: String, dst: String) -> R<()> {
        let store = Store::open(&self.dir, self.me.db_key())?;
        Ok(crate::export_file(&store, &peer_key, outgoing, &id, std::path::Path::new(&dst))?)
    }

    /// Empieza a recibir mensajes en segundo plano en `port` (y en el relé, si hay). `events` recibe
    /// un aviso cada vez que llega algo nuevo.
    pub fn listen(&self, port: u16, events: Option<Arc<dyn Events>>) -> R<()> {
        // [::] escucha IPv6 e IPv4 a la vez; si el teléfono no tiene IPv6, solo IPv4.
        let l = TcpListener::bind(("::", port)).or_else(|_| TcpListener::bind(("0.0.0.0", port))).map_err(|e| fail(e.to_string()))?;
        let (dir, me) = (self.dir.clone(), self.me.clone());
        // Además de la escucha directa, el buzón en el relé (si hay): así llegan las llamadas
        // aunque este teléfono esté detrás de un router o en datos móviles.
        let store = Store::open(&dir, me.db_key())?;
        // Puerto abierto en el router de casa (UPnP) + mi IP pública según el relé: así me llegan
        // directo desde otras redes, también desde datos móviles. Sin UPnP, se sigue con el relé.
        let relays = store.relays()?;
        thread::spawn(move || loop {
            let ruta = crate::upnp::open(port, 3600)
                .ok()
                .and_then(|public| relays.iter().find_map(|r| crate::relay::public_ip(r).ok()).map(|ip| format!("{ip}:{public}")));
            let ok = ruta.is_some();
            *PUBLIC.lock().unwrap_or_else(|e| e.into_inner()) = ruta;
            thread::sleep(Duration::from_secs(if ok { 30 * 60 } else { 10 * 60 }));
        });
        // Directorio más nuevo (relés y guardianes añadidos después de publicar la app).
        let (d, m) = (dir.clone(), me.clone());
        thread::spawn(move || Store::open(&d, m.db_key()).map(|s| crate::directorio::actualizar(&s)));
        let secret = store.mailbox_secret()?;
        for relay in store.relays()? {
            let (dir, me, events, secret) = (dir.clone(), me.clone(), events.clone(), secret.clone());
            thread::spawn(move || {
                crate::relay::listen(relay, secret, move |stream| {
                    if !OFFLINE.load(Ordering::SeqCst)
                        && let Some(_slot) = crate::Slot::take("rele:")
                        && let Ok(store) = Store::open(&dir, me.db_key())
                        && let Ok(n @ 1..) = serve(&store, &me, stream)
                        && let Some(e) = &events
                    {
                        e.received(n as u32);
                    }
                })
            });
        }
        thread::spawn(move || {
            crate::accept_loop(l, dir, me, || OFFLINE.load(Ordering::SeqCst), move |r| {
                // Los rechazos (desconocidos, impostores) simplemente cierran la conexión.
                if let (Ok(n @ 1..), Some(e)) = (r, &events) {
                    e.received(n as u32);
                }
            })
        });
        Ok(())
    }
}
