# Referencia de la API de MobileMesh

> **Archivo GENERADO. No lo edites a mano.** Se crea leyendo el código fuente.
> Para regenerarlo: `python3 tools/generar-referencia.py`
> Si cambias una función, vuelve a ejecutar el script; los cambios manuales se pierden.

Resumen: Android (funciones y propiedades): 66 · Web (métodos): 13 · React Native (métodos): 39 · CLI mesh (órdenes): 40 · MCP (herramientas): 8.

Los nombres de Android van en inglés (es la API). En web, React Native, CLI y MCP los nombres son en español, tal como están en el código.

## SDK de Android (Kotlin)

Todo vive en el paquete `org.mobilemesh.sdk` (`import org.mobilemesh.sdk.MobileMesh`). Descripciones: primera frase del KDoc tal como está en el código.

### `MobileMeshError`

`class MobileMeshError(message: String) : Exception(message)` — MobileMesh error, with a message ready to show (English, then Spanish in parentheses).

### `AccountState`

`enum class AccountState` — Account state on this phone.

Valores:

- `NO_ACCOUNT` — First time: `start` will create the account.
- `LOCKED` — The account exists, but the phone is locked: it opens once unlocked.
- `READY` — Ready to use.

### `Document`

`data class Document(val id: String, val fields: Map<String, Any?>, val conflicts: Map<String, List<Any?>>)` — A document of a collection.

### `PublicDocument`

`data class PublicDocument(val id: String, val author: String, val fields: Map<String, Any?>)` — Document of a public collection: `author` is the (verified) account that published it.

### `Delivery`

`data class Delivery(val count: Int, val inMailbox: Boolean)` — Result of a send: how many messages went out and whether they were left in the mailbox of a sleeping recipient.

### `Message`

`data class Message(val id: String, val contact: String, val sent: Boolean, val text: String, val status: String, val file: String?)` — A chat message (`sent`: true if outgoing).

### `MobileMesh`

`class MobileMesh` — MobileMesh for your app: database, accounts, messages, files, permissions and sync, with no servers.

| Firma | Qué hace |
|---|---|
| `val core: Mesh` | _(sin KDoc)_ |
| `fun MobileMesh.discreetNotice(context: Context, enabled: Boolean)` (estática) | Leaves the ongoing notification without a status-bar icon (it sits at the very bottom of the shade). |
| `fun MobileMesh.backgroundMode(context: Context, mode: BackgroundMode)` (estática) | How much MobileMesh works while the app is closed. |
| `fun MobileMesh.state(context: Context): AccountState` (estática) | Account state on this phone. |
| `fun MobileMesh.start(context: Context, port: Int = DEFAULT_PORT): MobileMesh` (estática) | Opens the user's account (creating it the first time) and leaves MobileMesh receiving in the background. |
| `fun MobileMesh.recover(context: Context, code: String, password: String, port: Int = DEFAULT_PORT): MobileMesh` (estática) | Recovers an account on a new phone with its recovery backup. |
| `fun MobileMesh.recoverWithGuardian(context: Context, username: String, password: String, port: Int = DEFAULT_PORT): Pair<MobileMesh, String>` (estática) | Lost phone: recovers the account with username and password from the guardian and brings back the data from its backup (if there was one). |
| `fun MobileMesh.recoverWithPaper(context: Context, code: String, port: Int = DEFAULT_PORT): Pair<MobileMesh, String>` (estática) | Forgotten password: recovers the account with the paper code (`paperCode`) and brings back the data from its backup in the guardian (if there was one). |
| `val myId: String` | Public identifier of the account. |
| `fun collection(name: String, strict: Boolean = false)` | Opens (or creates) a collection of documents. |
| `fun publicCollection(name: String)` | Public collection: what the app deliberately publishes for all its users to see (ranking, catalog, board). |
| `fun usePublicServer(invitation: String?)` | Uses your own public piece (its invitation) instead of the universal one; `null` goes back to the universal one. |
| `fun invitation(): String` | Invitation (text for a QR) with which another person adds you. |
| `fun addContact(name: String, invitation: String): Boolean` | Adds a contact with their invitation. |
| `fun contacts(): List<String>` | Names of your contacts. |
| `fun registerName(username: String)` | Registers (or updates) my username in the name guide so others can find me. |
| `fun userExists(username: String): Boolean` | Does this user exist? |
| `fun addByName(username: String)` | Adds a user by name and notifies them: their app will add me automatically (see `acceptRequests`). |
| `fun acceptRequests(): List<String>` | Adds as contacts those who want to talk to me. |
| `fun useNameGuide(invitation: String?)` | Uses another name guide (its invitation), or the universal one with `null`. |
| `fun block(contact: String, blocked: Boolean = true)` | Blocks (or unblocks, with `blocked = false`) a contact. |
| `fun sendMessage(contact: String, text: String, onDone: (Result<Delivery>) -> Unit = {})` | Sends a message. |
| `fun messages(): List<Message>` | All messages. |
| `fun sendFile(contact: String, file: Uri, name: String, onDone: (Result<Delivery>) -> Unit = {})` | Sends a file chosen by the user (photo, video, document). |
| `fun onReceive(listener: (count: Int) -> Unit): () -> Unit` | Notifies you (on the main thread) each time something new arrives. |
| `fun call(helper: String, function: String, data: Map<String, Any?> = emptyMap(), onDone: (Result<String>) -> Unit)` | Calls the function `function` of a helper (added as a contact with its invitation) and returns its result (text or JSON). |
| `fun recoveryBackup(password: String): String` | Recovery backup encrypted with `password` (at least 10 characters). |
| `fun paperCode(): String` | Paper code (56 letters, no password): recovers the account even if the password is forgotten (`MobileMesh.recoverWithPaper`). |
| `fun protectWithGuardian(username: String, password: String): String` | Protection against losing the phone: stores the account code in the guardian (encrypted with `password`, at least 12 characters) and an encrypted copy of the data. |
| `fun useRelay(address: String?)` | Uses your own relay "host:port" to reach other networks (applied when the app restarts). |
| `fun offlineMode(enabled: Boolean)` | Turns offline mode on or off. |
| `fun actAsSupernode(enabled: Boolean)` | Help other users: this phone acts as a relay (supernode) only on wifi and with battery (15% or more, or charging). |
| `fun isSupernode(): Boolean` | Is it acting as a supernode right now? |
| `fun checkMailboxEvery(minutes: Int)` | How often (in minutes) it checks the mailbox while the phone sleeps (1-60; 3 by default). |
| `fun instantReception(enabled: Boolean)` | Instant notices without Google, also with the screen off on phones that freeze apps (TECNO, ZTE, Xiaomi...): it keeps a silent sound playing so they do not freeze it. |
| `fun diagnostics(): String` | Summary of the state for debugging (no private data). |
| `fun checkDatabase(): List<String>` | Checks on request whether the database file is healthy. |

### `MeshCollection`

`class MeshCollection` — A collection of documents.

| Firma | Qué hace |
|---|---|
| `val name: String` | _(sin KDoc)_ |
| `fun save(id: String, fields: Map<String, Any?>)` | Saves fields of a document (text, numbers, booleans...). |
| `fun read(id: String): Document?` | Reads a document by id (`null` if it does not exist). |
| `fun all(): List<Document>` | All documents of the collection. |
| `fun where(field: String, operator: String, value: Any? = null)` | Query: `orders.where("price", ">", 10).where("paid", "==", false).sortBy("price").limit(20).get()`. |
| `fun sortBy(field: String, descending: Boolean = false)` | Starts a query sorted by `field`. |
| `fun delete(id: String)` | Deletes a document (on all devices once synced). |
| `fun subscribe(listener: (List<Document>) -> Unit): () -> Unit` | Calls `listener` with all the documents now and every time they change (by you, by your other devices or by whoever you share it with). |
| `fun shareWith(contact: String, canChange: Boolean)` | Shares the collection with a contact. |
| `fun unshareWith(contact: String)` | Stops sharing the collection with a contact. |
| `fun syncWith(contact: String, onDone: (Result<String>) -> Unit = {})` | Syncs now with a contact (it also happens automatically every few seconds). |

### `Query`

`class Query` — Query over a collection: chain it and finish with `get()` or `subscribe { }`.

| Firma | Qué hace |
|---|---|
| `fun where(field: String, operator: String, value: Any? = null)` | Adds a filter. |
| `fun sortBy(field: String, descending: Boolean = false)` | Sorts by `field`. |
| `fun limit(n: Int)` | At most `n` documents. |
| `fun get(): List<Document>` | Runs the query and returns the documents. |
| `fun subscribe(listener: (List<Document>) -> Unit): () -> Unit` | Like `MeshCollection.subscribe`, but only with the documents of the query. |

### `PublicCollection`

`class PublicCollection` — Public collection (see [MobileMesh.publicCollection]).

| Firma | Qué hace |
|---|---|
| `val name: String` | _(sin KDoc)_ |
| `fun publish(id: String, fields: Map<String, Any?>, onDone: (Result<Unit>) -> Unit = {})` | Publishes (or updates) document `id`, only if it is mine. |
| `fun delete(id: String, onDone: (Result<Unit>) -> Unit = {})` | Deletes a document I published. |
| `fun where(field: String, operator: String, value: Any? = null)` | Queries what everyone published: `ranking.sortBy("points", true).limit(10).get { ... }`. |
| `fun sortBy(field: String, descending: Boolean = false)` | Starts a query sorted by `field`. |
| `fun get(onDone: (Result<List<PublicDocument>>) -> Unit)` | Gets all the published documents (up to 50). |

### `PublicQuery`

`class PublicQuery` — Query over a public collection.

| Firma | Qué hace |
|---|---|
| `fun where(field: String, operator: String, value: Any? = null)` | Operators: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza`, `existe`. |
| `fun sortBy(field: String, descending: Boolean = false)` | Sorts by `field`. |
| `fun limit(n: Int)` | At most `n` documents (1-100; 50 by default). |
| `fun get(onDone: (Result<List<PublicDocument>>) -> Unit)` | Runs the query over the network and reports the documents with `onDone`. |

### `KeystoreGuard`

`class KeystoreGuard(private val context: Context) : KeyGuard` — Cifra la identidad con una clave AES que vive en Android Keystore: esa clave no se puede sacar del teléfono, así que copiar el archivo de identidad a otro equipo no sirve de nada.

### Declaraciones de nivel superior

| Firma | Qué hace |
|---|---|
| `const val DEFAULT_PORT = 7801` | Puerto por defecto; cada app puede elegir el suyo en `MobileMesh.start` (dos apps no pueden compartirlo). |

### `BackgroundMode`

`enum class BackgroundMode` — How much MobileMesh works in the background.

Valores:

- `FULL` — Servicio en primer plano + silencio + despertador: los mensajes llegan al instante.
- `ALARM` — Solo despertador: ni servicio ni notificación fija.
- `NONE` — Nada en segundo plano.

### `Permissions`

`object Permissions` — Pide al usuario, uno por uno, los permisos para que MobileMesh funcione al 100 % (recibir con la app cerrada y la pantalla apagada): avisos, batería sin restricciones e inicio automático del fabricante.

| Firma | Qué hace |
|---|---|
| `fun missing(c: Context): Boolean` | ¿Falta algo que se pueda comprobar? |
| `fun request(a: Activity)` | _(sin KDoc)_ |

_Piezas públicas internas no documentadas aquí: AvisoFijo, Avisos, BootReceiver, CopiaAutomatica, Despertador, DespertadorReceiver, MeshHolder, MeshService, ServicioFondo, Silencio, Supernodo._

**Alias obsoletos:** el SDK conserva 91 declaraciones en español marcadas `@Deprecated` (`guardar`, `coleccion`, `iniciar`, `EstadoCuenta`…). Siguen compilando, pero no son la API: no las uses en código nuevo.

## SDK web (JavaScript)

Archivo `web/mobilemesh.js`; se importa como módulo (`import { MobileMesh } from "./mobilemesh.js"`).

| Método | Qué hace |
|---|---|
| `static async start()` | Starts MobileMesh in this page: loads the core in a worker and connects the network. |
| `account()` | Identifier of this browser's account (created the first time and kept). |
| `save(collection, record, field, value)` | Saves one field of a record in a collection (encrypted and permanent). |
| `async all(collection)` | Records of a collection: [{id, campos: {…}}]. |
| `invitation()` | My invitation (so another person can add me as a contact). |
| `addContact(name, invitation)` | Adds a contact under a name, using the invitation they shared. |
| `sendMessage(name, text)` | Sends a text. |
| `async messages()` | All messages: [{contacto, saliente, texto, estado}]. |
| `onReceive(callback)` | Starts receiving (through the relay) while the page is open. |
| `async shareWith(collection, contact, permission)` | Shares a collection with a contact: "read", "write" or "none" (revoke). |
| `syncWith(contact)` | Syncs the shared collections with a contact. |
| `publicAddress(relay)` | My public IP as seen by a relay ("host:port"). |
| `async directory(relay)` | Signed directory of relays and guardians served by a relay (signature checked). |

## React Native

Se usa como `MobileMesh.<método>` tras `import { MobileMesh } from "mobilemesh"`.

| Método | Qué hace |
|---|---|
| `state()` | "sin cuenta" \| "bloqueado" \| "lista" |
| `start()` | Abre (o crea la primera vez) la cuenta de este móvil y pide los permisos de avisos. |
| `register(username, password)` | Tras `start`: registra el usuario (y guarda la cuenta en el guardián). |
| `signIn(username, password)` | En un móvil nuevo, en lugar de `start`: recupera la cuenta con usuario y contraseña. |
| `myUsername()` | _(sin comentario)_ |
| `paperCode()` | Código en papel (56 letras): enséñalo para apuntarlo; recupera la cuenta sin contraseña. |
| `recoverWithPaper(username, code, newPassword)` | Contraseña olvidada: entra con el código en papel y elige una contraseña nueva. |
| `userExists(username)` | _(sin comentario)_ |
| `isProtected()` | ¿Se puede recuperar la cuenta al formatear? |
| `protectWithGuardian(password)` | Protege la cuenta si al registrarse no se pudo. |
| `addByName(username)` | Añade a alguien por su nombre de usuario; su app te añadirá sola. |
| `acceptRequests()` | Acepta a quienes quieren hablar contigo. |
| `refresh()` | Actualiza tu invitación en la guía y acepta solicitudes. |
| `useNameGuide(invitation)` | _(sin comentario)_ |
| `invitation()` | Texto para que otro te añada (compártelo o ponlo en un QR). |
| `addContact(name, invitation)` | _(sin comentario)_ |
| `contacts()` | _(sin comentario)_ |
| `sendMessage(contact, text)` | Envía un texto. |
| `async messages()` | [{id, contacto, enviado, texto, estado, progreso, archivo, tipo, tamano, disponible}] |
| `onReceive(callback)` | Avisa con `callback(n)` cada vez que llega algo. |
| `pickFile()` | Selector de archivos de Android: {uri, nombre} o null. |
| `sendFile(contact, uri, name)` | _(sin comentario)_ |
| `viewFile(id)` | Ruta file:// descifrada en la caché (para <Image>). |
| `saveToDownloads(id)` | _(sin comentario)_ |
| `async call(helper, fn, data = {})` | Llama a una función de servidor de un ayudante (añádelo antes con `addByName`). |
| `async pay(producto, { ayudante = "pagos", esperarMin = 10 } = {})` | Cobrar con un ayudante de pagos (examples/ayudante-pagos): crea el pago, abre la pasarela, vuelve sola a la app al terminar y espera el resultado. |
| `save(collection, id, fields)` | Base de datos cifrada en el móvil. |
| `async all(collection)` | _(sin comentario)_ |
| `delete(collection, id)` | _(sin comentario)_ |
| `publish(collection, id, fields)` | Colecciones públicas: lo que la app publica a propósito para que lo vean todos (ranking, catálogo). |
| `deletePublished(collection, id)` | _(sin comentario)_ |
| `async queryPublic(collection, options = {})` | options: { donde: [["puntos", ">", 100]], orden: "puntos", desc: true, limite: 10 } |
| `useRelay(address)` | _(sin comentario)_ |
| `actAsSupernode(enabled)` | Ayudar a otros: este móvil hace de relé solo en wifi y con batería ≥15 % (encendido por defecto; `false` lo apaga). |
| `isSupernode()` | _(sin comentario)_ |
| `instantReception(enabled)` | Avisos al instante con la pantalla apagada y sin Google (encendido por defecto; se pausa por debajo del 15 % de batería). |
| `diagnostics()` | _(sin comentario)_ |
| `function formatCode(texto)` (también como `MobileMesh.formatCode`) | Para el campo donde se escribe el código en papel: mientras se teclea lo deja en grupos de 4 («2Y3Q-7KM9-…»), en mayúsculas, sin letras que no existen en el código y como mucho 56 letras. |
| `function formatearCodigo(texto)` (también como `MobileMesh.formatearCodigo`) | _(sin comentario)_ |

## Línea de órdenes `mesh`

Se invoca como `mesh <carpeta> <orden> …` (`<carpeta>` es la del dispositivo simulado).

| Orden | Qué hace |
|---|---|
| `mesh <carpeta> init` | crear identidad y base de datos |
| `mesh <carpeta> id` | ver mi clave pública (para compartir) |
| `mesh <carpeta> invitacion <ip:puerto>` | ver mi invitación (lo que va en el QR) |
| `mesh <carpeta> contacto <nombre> <clave> [ip:puerto]` | añadir un contacto de confianza |
| `mesh <carpeta> contacto <nombre> <invitación>` | añadir un contacto de confianza |
| `mesh <carpeta> escuchar <ip:puerto>` | recibir mensajes |
| `mesh <carpeta> ayudante <ip:puerto> <programa>` | funciones de servidor: atiende llamadas de cualquiera ejecutando «<programa> <función>» (recibe {de, datos} por la entrada y devuelve el resultado por la salida) |
| `mesh <carpeta> enviar <nombre> <ip:puerto> <texto>` | _(sin descripción en USO)_ |
| `mesh <carpeta> reintentar <nombre> <ip:puerto>` | reenviar los pendientes |
| `mesh <carpeta> mensajes` | ver mensajes y su estado |
| `mesh <carpeta> archivo <nombre> <ip:puerto> <ruta>` | enviar un archivo |
| `mesh <carpeta> exportar <id> <destino>` | descifrar un archivo recibido o enviado |
| `mesh <carpeta> aceptar-copias <nombre>` | guardar copias cifradas de un contacto |
| `mesh <carpeta> copias` | ver las copias que guardo para otros |
| `mesh <carpeta> usar-rele <ip:puerto>` | recibir también a través de un relé |
| `mesh <carpeta> buzon` | recoger mensajes y acuses dejados en mi buzón |
| `mesh <carpeta> sincronizar <ruta>` | sincronizar con otro dispositivo de mi cuenta |
| `mesh <carpeta> compartir <colección> <nombre> leer\|escribir\|nada` | _(sin descripción en USO)_ |
| `mesh <carpeta> sincronizar-con <nombre> <ruta>` | sincronizar lo compartido con un contacto |
| `mesh <carpeta> dato <colección> <registro> <campo> <valor>` | _(sin descripción en USO)_ |
| `mesh <carpeta> datos <colección>` | ver una colección |
| `mesh <carpeta> copia` | crear copia de recuperación (pide contraseña) |
| `mesh <carpeta> directorio` | buscar en mis relés relés y guardianes nuevos |
| `mesh <carpeta> guardian <ip:puerto> <ruta pública>` | ser guardián: guardar códigos y copias cifradas de cualquiera |
| `mesh <carpeta> nombres <ip:puerto> <ruta pública>` | ser guía de nombres: registrar y buscar usuarios por nombre |
| `mesh <carpeta> publico <ip:puerto> <ruta pública>` | ser pieza pública: lo que las apps publican para todos |
| `mesh <carpeta> publicar <pieza> <colección> <id> <json>` | publicar un documento para todos |
| `mesh <carpeta> despublicar <pieza> <colección> <id>` | borrar un documento mío |
| `mesh <carpeta> consultar-publico <pieza> <colección> [orden]` | lo publicado por todos (mayor primero) |
| `mesh <carpeta> app <paquete>` | a qué app pertenecen mis nombres (com.empresa.app) |
| `mesh <carpeta> registrar-nombre <guía> <nombre> <mi ruta>` | registrar mi nombre en una guía |
| `mesh <carpeta> buscar-nombre <guía> <nombre>` | invitación de un nombre |
| `mesh <carpeta> pedir-contacto <guía> <nombre>` | avisar a un nombre de que quiero hablar con él |
| `mesh <carpeta> solicitudes <guía>` | añadir como contactos a quienes quieren hablar conmigo |
| `mesh <carpeta> guardar-cuenta <invitación> <usuario>` | guardar mi código en un guardián (pide contraseña) |
| `mesh <carpeta> buscar-cuenta <invitación> <usuario>` | teléfono nuevo: recuperar la cuenta desde un guardián |
| `mesh <carpeta> restaurar <código>` | recuperar la cuenta (pide contraseña) |
| `mesh <carpeta> vincular-pedir` | dispositivo nuevo: mostrar su solicitud |
| `mesh <carpeta> vincular-aprobar <solicitud>` | principal: aprobarla y mostrar el certificado |
| `mesh <carpeta> vincular-completar <certificado>` | dispositivo nuevo: instalar el certificado |

## Servidor MCP `mesh-mcp`

Herramientas que expone a una IA (`tools/list`). El nodo de prueba vive dentro de `~/.mobilemesh-mcp`.

| Herramienta | Parámetros | Qué hace |
|---|---|---|
| `guia` | — | Guía completa del SDK de Android de MobileMesh (Kotlin): cuenta, colecciones, permisos, archivos, mensajes, ayudantes, relés y recuperación. |
| `crear_nodo` | `carpeta` | Crea un nodo de prueba (identidad y base de datos cifrada) en una carpeta. |
| `guardar_dato` | `carpeta`, `coleccion`, `registro`, `campo`, `valor` | Guarda un campo de un registro en una colección del nodo (se cifra y se sincroniza con quien la comparta). |
| `ver_datos` | `carpeta`, `coleccion` | Lista los registros de una colección del nodo con sus campos. |
| `invitacion` | `carpeta`, `direccion` | Invitación del nodo (lo que va en el QR) para que otro lo añada como contacto. |
| `anadir_contacto` | `carpeta`, `nombre`, `invitacion` | Añade un contacto de confianza con su invitación. |
| `dar_permiso` | `carpeta`, `coleccion`, `contacto`, `permiso` | Comparte una colección con un contacto: leer, escribir o nada (quitar). Se comprueba en cada cambio que llega. |
| `estado_red` | — | Relés y guardianes públicos (del directorio firmado) y si responden ahora. |
