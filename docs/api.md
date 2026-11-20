# Contrato común de la API de MobileMesh

Este documento es el contrato de la API pública de MobileMesh: todos los SDK (Android, web, React Native y los que se añadan) deben ofrecer las mismas operaciones con los mismos nombres y el mismo significado. La referencia detallada por plataforma, generada del código, sigue siendo [`docs/referencia-api.md`](referencia-api.md); este contrato no la sustituye ni la duplica.

## 1. Reglas de nomenclatura

- Los nombres de la API van en inglés: `start`, `save`, `read`, `all`, `subscribe`, `sendMessage`, `shareWith`…
- Cada lenguaje usa su estilo: `camelCase` en Kotlin, JavaScript, Dart y Swift; `snake_case` en Rust y Python si se añaden.
- Los nombres en español que ya existen se mantienen solo como alias obsoletos (`guardar`, `coleccion`, `iniciar`, `enviar`…). Siguen funcionando, avisan una vez y no deben usarse en código nuevo.
- Una operación nueva se añade primero a este contrato y después a cada SDK; no se crean nombres distintos por plataforma.

## 2. El contrato por áreas

Convención de las tablas: la firma canónica es la de Android cuando la operación nace allí; si otra plataforma cambia los argumentos, se indica en la misma celda. Los errores entre comillas invertidas remiten a la tabla de la sección 5.

### Arranque y cuenta

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `start` | Android: `MobileMesh.start(context, port = 7801)`<br>Web: `await MobileMesh.start({ appId })`<br>RN: `start()` | Instancia de MobileMesh (Android/web) o id de la cuenta (RN) | `FALTA_APPID` (web sin `appId` ni dominio), `TELEFONO_BLOQUEADO`, error de cabeceras COOP/COEP (web) | Android, Web, RN |
| `state` | `MobileMesh.state(context)` · RN: `state()` | `AccountState` (`NO_ACCOUNT`, `LOCKED`, `READY`) o `"sin cuenta"`, `"bloqueado"`, `"lista"` | — | Android, RN |
| `account` | Web: `account()` · Android: propiedad `myId` | Identificador público de la cuenta | — | Android (como `myId`), Web |
| `backgroundMode` | `MobileMesh.backgroundMode(context, BackgroundMode.{FULL,ALARM,NONE})` | `Unit` | Debe llamarse antes de `start` | Solo Android |
| `Permissions.request` | `Permissions.request(activity)` | `Unit` | — | Solo Android |
| `register` | RN: `register(username, password)` | Aviso o `null` | `SIN_RELE` si no se pudo guardar en el guardián | Solo RN |
| `recover` | `MobileMesh.recover(context, code, password, port = 7801)` | Instancia de MobileMesh | Copia inválida o contraseña incorrecta | Solo Android |

### Colecciones y documentos

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `collection` | `collection(name, strict = false)` | `MeshCollection` | `BASE_OTRA_APP` al abrir una base de otra marca | Solo Android |
| `save` | Android: `collection(name).save(id, fields)`<br>RN: `save(collection, id, fields)`<br>Web: `save(collection, record, field, value)` | `Unit` / promesa resuelta | `BASE_OTRA_APP`; web: valor normal limitado a 8 KB | Android, Web (plano), RN |
| `read` | `collection(name).read(id)` | `Document?` (`null` si no existe) | — | Solo Android |
| `all` | Android: `collection(name).all()`<br>Web: `await all(collection)`<br>RN: `await all(collection)` | `List<Document>` / `[{id, campos}]` | — | Android, Web, RN |
| `delete` | Android: `collection(name).delete(id)`<br>RN: `delete(collection, id)` | `Unit` | — | Android, RN |
| `saveLarge` | `saveLarge(collection, record, field, value)` | Promesa resuelta | — | Solo Web |
| `readLarge` | `readLarge(collection, record, field)` | Valor completo; cadena vacía si no está | — | Solo Web |

### Consultas

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `where` | `collection(name).where(field, operator, value = null)` | `Query` encadenable | Operador no válido | Solo Android |
| `sortBy` | `query.sortBy(field, descending = false)` | `Query` | — | Solo Android |
| `limit` | `query.limit(n)` | `Query` | — | Solo Android |
| `get` | `query.get()` | `List<Document>` | — | Solo Android |
| `subscribe` (consulta) | `query.subscribe(listener)` | Función para cancelar | — | Solo Android |

Operadores admitidos: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza`, `existe`.

### Tiempo real

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `subscribe` (colección) | `collection(name).subscribe(listener)` | Función para cancelar | — | Solo Android |

El aviso de mensajes nuevos es `onReceive`, en el área de mensajes. Web no tiene suscripción a colecciones.

### Mensajes

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `sendMessage` | Android: `sendMessage(contact, text, onDone)`<br>Web: `sendMessage(name, text)`<br>RN: `sendMessage(contact, text)` | `Delivery(count, inMailbox)`; web/RN: `"entregado"`, `"en buzón"` o `"pendiente: …"` | `SIN_RELE` si no hay relé y no se puede dejar en buzón | Android, Web, RN |
| `messages` | `messages()` | `List<Message>` / `[{contacto, saliente, texto, estado}]` | — | Android, Web, RN |
| `onReceive` | `onReceive(listener)` | Android: función para cancelar; web: `undefined`; RN: función para cancelar | — | Android, Web, RN |
| `call` | Android: `call(helper, function, data = {}, onDone)`<br>RN: `call(helper, fn, data = {})` | `String` o resultado JSON | El ayudante no es contacto o no responde | Android, RN |

### Archivos

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `sendFile` | Android: `sendFile(contact, file: Uri, name, onDone)`<br>RN: `sendFile(contact, uri, name)` | `Delivery` / promesa resuelta | Archivo ilegible; `SIN_RELE` | Android, RN |
| `pickFile` | RN: `pickFile()` | `{uri, nombre}` o `null` | — | Solo RN |
| `viewFile` | RN: `viewFile(id)` | Ruta `file://` descifrada en caché | Archivo no disponible | Solo RN |
| `saveToDownloads` | RN: `saveToDownloads(id)` | Promesa resuelta | Archivo no disponible | Solo RN |

### Contactos e invitaciones

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `invitation` | `invitation()` | Texto de invitación (para QR) | `FALTA_APPID` si no hay marca | Android, Web, RN |
| `addContact` | `addContact(name, invitation)` | Android: `Boolean`; web/RN: promesa resuelta | `INVITACION_OTRA_APP`, `INVITACION_VIEJA` | Android, Web, RN |
| `contacts` | `contacts()` | `List<String>` con los nombres | — | Android, RN |
| `block` | `block(contact, blocked = true)` | `Unit` | — | Solo Android |

### Permisos de compartición

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `shareWith` | Android: `collection(name).shareWith(contact, canChange)`<br>Web: `shareWith(collection, contact, permission)` con `"read"`, `"write"` o `"none"` | `Unit` / promesa resuelta | Contacto inexistente; web: permiso inválido | Android, Web |
| `unshareWith` | `collection(name).unshareWith(contact)` | `Unit` | — | Solo Android (en web se usa `shareWith(..., "none")`) |

### Sincronización

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `syncWith` | Android: `collection(name).syncWith(contact, onDone)`<br>Web: `syncWith(contact)` | `Result<String>` / promesa resuelta | Contacto inexistente; `SIN_RELE` | Android, Web |

La sincronización automática ocurre sola cada pocos segundos; `syncWith` solo fuerza el momento.

### Colecciones públicas

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `publicCollection` | `publicCollection(name)` | `PublicCollection` | — | Solo Android |
| `publish` | Android: `publicCollection(name).publish(id, fields, onDone)`<br>RN: `publish(collection, id, fields)` | `Result<Unit>` / promesa resuelta | El documento no es mío; `SIN_RELE` | Android, RN |
| `deletePublished` | Android: `publicCollection(name).delete(id, onDone)`<br>RN: `deletePublished(collection, id)` | `Result<Unit>` / promesa resuelta | El documento no es mío | Android, RN |
| `queryPublic` | Android: `publicCollection(name).where(...).sortBy(...).limit(...).get(onDone)`<br>RN: `queryPublic(collection, options)` | `List<PublicDocument>` / documentos publicados | Consulta inválida; `SIN_RELE` | Android, RN |
| `usePublicServer` | `usePublicServer(invitation?)`; `null` vuelve al universal | `Unit` | `INVITACION_OTRA_APP` | Solo Android |

`queryPublic` acepta en RN `{ donde, orden, desc, limite }`; el máximo es 50 documentos (100 con `limit`).

### Guía de nombres

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `registerName` | Android: `registerName(username)`<br>RN: `register(username, password)` | `Unit` / aviso o `null` | Nombre no disponible; `SIN_RELE` | Android, RN |
| `userExists` | `userExists(username)` | `Boolean` | — | Android, RN |
| `addByName` | `addByName(username)` | `Unit` | El nombre no existe | Android, RN |
| `acceptRequests` | `acceptRequests()` | `List<String>` con los nombres aceptados | — | Android, RN |
| `useNameGuide` | `useNameGuide(invitation?)`; `null` vuelve a la universal | `Unit` | `INVITACION_OTRA_APP` | Android, RN |
| `refresh` | RN: `refresh()` | Promesa resuelta | `SIN_RELE` | Solo RN |
| `myUsername` | RN: `myUsername()` | Nombre propio o `null` | — | Solo RN |

### Copias de recuperación

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `recoveryBackup` | `recoveryBackup(password)` | Texto de copia cifrada | Contraseña de menos de 10 caracteres | Solo Android |
| `paperCode` | `paperCode()` | Código en papel de 56 letras | — | Android, RN |
| `protectWithGuardian` | Android: `protectWithGuardian(username, password)`<br>RN: `protectWithGuardian(password)` | Texto de protección / promesa resuelta | Contraseña de menos de 12 caracteres (Android); `SIN_RELE` | Android, RN |
| `isProtected` | RN: `isProtected()` | `Boolean` | — | Solo RN |
| `recoverWithGuardian` | Android: `MobileMesh.recoverWithGuardian(context, username, password)`<br>RN: `signIn(username, password)` | Instancia y aviso / promesa resuelta | Credenciales incorrectas; `TELEFONO_BLOQUEADO` | Android, RN |
| `recoverWithPaper` | Android: `MobileMesh.recoverWithPaper(context, code)`<br>RN: `recoverWithPaper(username, code, newPassword)` | Instancia y aviso / promesa resuelta | Código inválido | Android, RN |

### Red y relés

| Operación | Firma canónica | Devuelve | Errores | Plataformas |
|---|---|---|---|---|
| `useRelay` | `useRelay(address?)`; `null` quita el relé propio | `Unit` | Dirección inválida | Android, RN |
| `publicAddress` | Web: `publicAddress(relay)` | IP pública vista por el relé | `SIN_RELE` | Solo Web |
| `directory` | Web: `await directory(relay)` | Directorio firmado de relés y guardianes | Firma inválida; `SIN_RELE` | Solo Web |
| `offlineMode` | `offlineMode(enabled)` | `Unit` | — | Solo Android |
| `actAsSupernode` | `actAsSupernode(enabled)` | `Unit` | — | Android, RN |
| `isSupernode` | `isSupernode()` | `Boolean` | — | Android, RN |
| `checkMailboxEvery` | `checkMailboxEvery(minutes)` (1-60) | `Unit` | Valor fuera de rango | Solo Android |
| `instantReception` | `instantReception(enabled)` | `Unit` | — | Android, RN |
| `diagnostics` | `diagnostics()` | Resumen de texto sin datos privados | — | Android, RN |
| `checkDatabase` | `checkDatabase()` | `List<String>` con el resultado | — | Solo Android |

## 3. Cobertura por plataforma

✅ existe hoy · ❌ falta. Node, Flutter y Swift todavía no tienen SDK.

| Operación | Android/Kotlin | Web/JS | React Native | Node | Flutter | Swift |
|---|---|---|---|---|---|---|
| `start` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `state` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `account` | ✅ (`myId`) | ✅ | ❌ | ❌ | ❌ | ❌ |
| `backgroundMode` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `Permissions.request` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `register` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `recover` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `collection` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `save` | ✅ | ✅ (plano) | ✅ | ❌ | ❌ | ❌ |
| `read` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `all` | ✅ | ✅ (plano) | ✅ (plano) | ❌ | ❌ | ❌ |
| `delete` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `saveLarge` | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `readLarge` | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `where` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `sortBy` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `limit` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `get` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `subscribe` (consulta) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `subscribe` (colección) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `sendMessage` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `messages` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `onReceive` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `call` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `sendFile` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `pickFile` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `viewFile` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `saveToDownloads` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `invitation` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `addContact` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `contacts` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `block` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `shareWith` | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `unshareWith` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `syncWith` | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `publicCollection` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `publish` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `deletePublished` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `queryPublic` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `usePublicServer` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `registerName` | ✅ | ❌ | ✅ (`register`) | ❌ | ❌ | ❌ |
| `userExists` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `addByName` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `acceptRequests` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `useNameGuide` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `refresh` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `myUsername` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `recoveryBackup` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `paperCode` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `protectWithGuardian` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `isProtected` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `recoverWithGuardian` | ✅ | ❌ | ✅ (`signIn`) | ❌ | ❌ | ❌ |
| `recoverWithPaper` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `useRelay` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `publicAddress` | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `directory` | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `offlineMode` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `actAsSupernode` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `isSupernode` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `checkMailboxEvery` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `instantReception` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `diagnostics` | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `checkDatabase` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |

## 4. Huecos conocidos

- **Web es plano**: no tiene `collection` ni consultas. Guarda campo a campo con `save(colección, id, campo, valor)`, lee con `all(colección)` y usa `saveLarge`/`readLarge` para valores grandes. Android sí tiene `collection().save/read/all/where/sortBy/limit/subscribe`.
- **React Native** hereda el modelo de Android pero no expone colecciones tipadas ni consultas locales: usa `save(colección, id, campos)`, `all`, `delete` y solo consulta colecciones públicas con `queryPublic`.
- **React Native** conserva nombres en español como alias obsoletos; la API nueva es la inglesa.
- No hay SDK de Node, Flutter ni Swift. Sus columnas de la tabla son el mapa de trabajo pendiente.
- Web no tiene archivos, ayudantes (`call`), guía de nombres, copias de recuperación ni relé propio configurable.
- Android no tiene `account()` como método: el identificador público está en la propiedad `myId`.

## 5. Errores comunes

Todo SDK debe devolver estos casos con el mismo significado. El texto entre paréntesis es el mensaje de referencia en español; puede ir acompañado de su traducción al inglés.

| Código | Mensaje de referencia | Cuándo |
|---|---|---|
| `INVITACION_OTRA_APP` | «la invitación es de otra aplicación» | La invitación se generó con otra marca (`appId`). Solo se comunican copias con la misma marca. |
| `BASE_OTRA_APP` | «esta base de datos pertenece a otra aplicación» | La base ya existe con otra marca. No se borra ni se cambia la marca. |
| `FALTA_APPID` | «falta el identificador de aplicación (appId)» | No hay marca: falta `mobilemesh.properties` en Android o `appId` en web. |
| `INVITACION_VIEJA` | «invitación de una versión anterior» | La invitación es de un protocolo viejo; hay que generar una nueva. |
| `TELEFONO_BLOQUEADO` | «el teléfono está bloqueado» | La cuenta existe pero no se abre hasta desbloquear el teléfono. Se consulta antes con `state`. |
| `SIN_RELE` | «no hay relé configurado» | La operación necesita relé y no hay ninguno disponible ni buzón donde dejar el mensaje. |
