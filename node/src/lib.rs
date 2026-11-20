//! Binding de MobileMesh para Node.js mediante napi-rs.

use napi_derive::napi;
use std::{
    collections::HashMap,
    sync::Arc,
};

use mobilemesh_core::ffi::{KeyGuard, Mesh as CoreMesh, MeshError, QueryFilter, SyncRecord};

/// En un servidor no hay llavero del sistema: la identidad se protege con los
/// permisos del archivo que la guarda, así que aquí no se transforma la clave.
struct FileKeyGuard;

impl KeyGuard for FileKeyGuard {
    fn seal(&self, plain: Vec<u8>) -> Result<Vec<u8>, MeshError> {
        Ok(plain)
    }

    fn open(&self, sealed: Vec<u8>) -> Result<Vec<u8>, MeshError> {
        Ok(sealed)
    }
}

fn raise(e: MeshError) -> napi::Error {
    napi::Error::from_reason(e.to_string())
}

/// Convierte un documento del núcleo al JSON que se devuelve a JavaScript.
fn record_json(record: SyncRecord) -> serde_json::Value {
    let fields: serde_json::Map<String, serde_json::Value> = record
        .fields
        .into_iter()
        .map(|field| (field.name, serde_json::Value::String(field.value)))
        .collect();
    let conflicts: Vec<serde_json::Value> = record
        .conflicts
        .into_iter()
        .map(|conflict| {
            serde_json::json!({
                "field": conflict.field,
                "values": conflict.values,
            })
        })
        .collect();
    serde_json::json!({
        "id": record.id,
        "fields": fields,
        "conflicts": conflicts,
    })
}

#[napi]
pub struct Mesh {
    inner: Arc<CoreMesh>,
}

#[napi]
impl Mesh {
    /// Abre la cuenta si ya existe en `dir`; si no, la crea.
    #[napi(factory)]
    pub fn start(dir: String, app_id: String) -> napi::Result<Mesh> {
        let guard: Arc<dyn KeyGuard> = Arc::new(FileKeyGuard);
        let inner = match CoreMesh::open(dir.clone(), app_id.clone(), guard.clone()) {
            Ok(mesh) => mesh,
            Err(_) => CoreMesh::create(dir, app_id, guard).map_err(raise)?,
        };
        Ok(Mesh { inner })
    }

    #[napi]
    pub fn account_id(&self) -> String {
        self.inner.id()
    }

    /// Guarda cada campo con la política de «gana el último cambio».
    #[napi]
    pub fn save(
        &self,
        collection: String,
        id: String,
        fields: HashMap<String, String>,
    ) -> napi::Result<()> {
        for (field, value) in fields {
            self.inner
                .put(collection.clone(), id.clone(), field, Some(value), false)
                .map_err(raise)?;
        }
        Ok(())
    }

    /// Devuelve todos los documentos de la colección como JSON.
    #[napi]
    pub fn all(&self, collection: String) -> napi::Result<String> {
        let records = self.inner.records(collection).map_err(raise)?;
        let docs: Vec<serde_json::Value> = records
            .into_iter()
            .map(|record| {
                let fields: serde_json::Map<String, serde_json::Value> = record
                    .fields
                    .into_iter()
                    .map(|field| (field.name, serde_json::Value::String(field.value)))
                    .collect();
                let conflicts: Vec<serde_json::Value> = record
                    .conflicts
                    .into_iter()
                    .map(|conflict| {
                        serde_json::json!({
                            "field": conflict.field,
                            "values": conflict.values,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": record.id,
                    "fields": fields,
                    "conflicts": conflicts,
                })
            })
            .collect();
        Ok(serde_json::Value::Array(docs).to_string())
    }

    #[napi]
    pub fn invitation(&self, address: String) -> napi::Result<String> {
        self.inner.invite(address).map_err(raise)
    }

    /// Devuelve un documento como JSON, o `null` si no existe.
    #[napi]
    pub fn read(&self, collection: String, id: String) -> napi::Result<Option<String>> {
        let records = self.inner.records(collection).map_err(raise)?;
        Ok(records.into_iter().find(|record| record.id == id).map(|record| record_json(record).to_string()))
    }

    /// Consulta la colección con filtros en JSON y devuelve los documentos como JSON.
    #[napi]
    pub fn query(
        &self,
        collection: String,
        filters_json: String,
        order_by: Option<String>,
        descending: Option<bool>,
        limit: Option<u32>,
    ) -> napi::Result<String> {
        let filters: Vec<QueryFilter> = serde_json::from_str::<Vec<serde_json::Value>>(&filters_json)
            .map_err(|e| napi::Error::from_reason(format!("filtros no válidos: {e}")))?
            .into_iter()
            .map(|filter| QueryFilter {
                field: filter["field"].as_str().unwrap_or_default().to_string(),
                op: filter["op"].as_str().unwrap_or_default().to_string(),
                value: match &filter["value"] {
                    serde_json::Value::String(text) => text.clone(),
                    value => value.to_string(),
                },
            })
            .collect();
        let records = self
            .inner
            .query(collection, filters, order_by, descending.unwrap_or(false), limit)
            .map_err(raise)?;
        Ok(serde_json::Value::Array(records.into_iter().map(record_json).collect()).to_string())
    }

    /// Añade un contacto desde su invitación, validando la marca de la app.
    #[napi]
    pub fn add_contact(&self, name: String, invitation: String) -> napi::Result<bool> {
        self.inner.add_contact_from_invitation(name, invitation).map_err(raise)
    }

    /// Devuelve los contactos como JSON.
    #[napi]
    pub fn contacts(&self) -> napi::Result<String> {
        let contacts: Vec<serde_json::Value> = self
            .inner
            .contacts()
            .map_err(raise)?
            .into_iter()
            .map(|contact| {
                serde_json::json!({
                    "name": contact.name,
                    "key": contact.key,
                    "addr": contact.addr,
                })
            })
            .collect();
        Ok(serde_json::Value::Array(contacts).to_string())
    }

    /// Envía un mensaje a un contacto y devuelve el resultado como JSON.
    #[napi]
    pub fn send_message(&self, name: String, text: String) -> napi::Result<String> {
        let result = self.inner.send(name, text).map_err(raise)?;
        Ok(serde_json::json!({
            "count": result.count,
            "in_mailbox": result.in_mailbox,
        })
        .to_string())
    }

    /// Devuelve los mensajes recibidos como JSON.
    #[napi]
    pub fn messages(&self) -> napi::Result<String> {
        let messages: Vec<serde_json::Value> = self
            .inner
            .messages()
            .map_err(raise)?
            .into_iter()
            .map(|message| {
                serde_json::json!({
                    "id": message.id,
                    "peer": message.peer,
                    "peer_key": message.peer_key,
                    "outgoing": message.outgoing,
                    "body": message.body,
                    "status": message.status,
                    "file_name": message.file_name,
                    "file_size": message.file_size,
                    "mime": message.mime,
                    "progress": message.progress,
                    "file_available": message.file_available,
                })
            })
            .collect();
        Ok(serde_json::Value::Array(messages).to_string())
    }

    /// Comparte una colección con un contacto, con o sin permiso de escritura.
    #[napi]
    pub fn share_with(&self, collection: String, name: String, can_write: bool) -> napi::Result<()> {
        self.inner.share(collection, name, Some(can_write)).map_err(raise)
    }

    /// Sincroniza ahora lo compartido con un contacto y devuelve el resumen.
    #[napi]
    pub fn sync_with(&self, name: String) -> napi::Result<String> {
        self.inner.sync_with_contact(name).map_err(raise)
    }

    /// Recoge los mensajes del buzón del relé y devuelve cuántos llegaron.
    #[napi]
    pub fn check_mailbox(&self) -> napi::Result<u32> {
        self.inner.check_mailbox().map_err(raise)
    }

    /// Configura el relé («host:puerto») o lo quita con `null`.
    #[napi]
    pub fn set_relay(&self, relay: Option<String>) -> napi::Result<()> {
        self.inner.set_relay(relay).map_err(raise)
    }

    /// Devuelve el relé configurado, o `null` si no hay.
    #[napi]
    pub fn relay(&self) -> napi::Result<Option<String>> {
        self.inner.relay_info().map_err(raise)
    }
}
