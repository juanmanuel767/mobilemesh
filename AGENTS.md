# MobileMesh para agentes de código

## Qué es
- Base de datos y mensajería entre dispositivos **sin servidores**: cada usuario guarda sus datos cifrados en su teléfono y los comparte cifrados de extremo a extremo con sus contactos.
- SDK de Android (Kotlin), puente React Native, SDK web (WASM + JS), CLI `mesh` y servidor MCP `mesh-mcp`.
- **NO es** un servidor de datos: no hay base central ni se puede buscar o consultar entre datos de distintos usuarios. Cada consulta corre en el teléfono sobre sus propias colecciones (o las que le compartieron). Lo que debe verlo todo el mundo (ranking, catálogo) se publica a propósito en una colección pública.

## Requisitos duros de Android
- `compileSdk` 35 o superior · `minSdk` 28 · JDK 17 · Kotlin 2.2.0 o superior.
- Combinación probada desde un proyecto nuevo: AGP 8.7.3, Gradle 8.9, Kotlin 2.2.0, `compileSdk` 35.

## Dependencia
```kotlin
// settings.gradle.kts
dependencyResolutionManagement { repositories { google(); mavenCentral(); maven("https://jitpack.io") } }
// build.gradle.kts de tu app
dependencies { implementation("com.github.juanmanuel767:mobilemesh:v0.2.1") }
```
Todos los tipos viven en el paquete `org.mobilemesh.sdk`: hay que importarlos de ahí.

## Ejemplo mínimo
```kotlin
import android.content.Context
import org.mobilemesh.sdk.MobileMesh

fun demo(context: Context) {
    val mesh = MobileMesh.start(context) // crea la cuenta la primera vez
    val orders = mesh.collection("orders")
    orders.save("p1", mapOf("product" to "bread", "quantity" to 2, "paid" to false))
    val quantity = orders.read("p1")?.fields?.get("quantity") // 2
}
```

## Avisos y segundo plano
Elige el modo **antes** de `start`: `MobileMesh.backgroundMode(context, BackgroundMode.ALARM)`. Por defecto es `FULL`.

| Modo | Qué cuesta |
|---|---|
| `FULL` | Mensajes al instante. Notificación fija y tres permisos que Google Play restringe (`FOREGROUND_SERVICE_REMOTE_MESSAGING`, `SCHEDULE_EXACT_ALARM`, `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`) que hay que justificar en la ficha de Play. |
| `ALARM` | Android despierta la app cada pocos minutos, mira el buzón del relé y avisa. Ningún permiso restringido ni notificación fija; los avisos llegan con unos minutos de retraso. |
| `NONE` | Nada en segundo plano; leer y escribir datos sigue igual. No recibe nada con la app cerrada. |

- Si la app no es de mensajería, `ALARM` es casi siempre lo que quieres: evita la revisión de Play por permisos restringidos.
- Avisos al instante **sin Google**: `mesh.instantReception(enabled)`, encendido por defecto (mantiene un sonido en silencio para que el fabricante no congele la app; se pausa con batería por debajo del 15 % sin cargar). Con el destino dormido el mensaje queda cifrado en su buzón del relé y lo recoge al despertar (`mesh.checkMailboxEvery(minutes)`).
- Avisos al instante **con Firebase** (opcional): pieza `mobilemesh-avisos-fcm`. Hoy no se publica en JitPack: exige clonar el repositorio y compilarla (detalles en `docs/sdk.md`).

## Trampas
- Sin Kotlin 2.2.0 falla con «was compiled with an incompatible version of Kotlin».
- El puerto de `start` es opcional (`MobileMesh.start(context)`). Solo se da cuando dos apps del mismo teléfono usan MobileMesh.
- `Document.fields` es `Map<String, Any?>`: hay que convertir al tipo guardado (p. ej. `as? Int`).
- La clase de colecciones es `MeshCollection` (no `Collection`, que sombrearía la de Kotlin).
- Para recibir al 100 % con la app cerrada hace falta llamar a `Permissions.request(this)` (en `onResume`).
- Los nombres en español (`guardar`, `coleccion`, `iniciar`…) son alias `@Deprecated` en Android: usa los ingleses.
- La marca de la app es obligatoria y se genera sola: añade `apply(from = "../../tools/mobilemesh.gradle.kts")` al `build.gradle.kts` (ajusta la ruta). Crea `src/main/assets/mobilemesh.properties` (`appId=mm-<32 hex>`): guárdalo y no lo borres. Si falta, se usa el nombre del paquete.
- Las invitaciones v2 (`mobilemesh:2:…`) ya no valen: el protocolo v3 lleva la marca. Una invitación de otra app falla con «la invitación es de otra aplicación», y abrir una base con otra marca, con «esta base de datos pertenece a otra aplicación».

## Marca de la app (obligatoria)

Cada app tiene una marca (App ID). Solo las copias con la misma marca se comunican: una invitación de otra app se rechaza, y una base creada con otra marca no se abre.

- **Android**: se genera sola al compilar. Añade `apply(from = "../../tools/mobilemesh.gradle.kts")` al `build.gradle.kts` de tu app (ajusta la ruta). La primera compilación crea `src/main/assets/mobilemesh.properties` con `appId=mm-<32 hex>`. Ese archivo se guarda en el repositorio y viaja dentro del APK. Si falta, se usa el nombre del paquete.
- **Web**: `await MobileMesh.start({ appId: "mi-web" })`. Si no indicas `appId`, se usa el dominio de la página. No la cambies una vez publicada: la base del navegador se guarda por marca.
- **React Native**: hereda la marca de Android; no se configura aparte.
- La marca **nunca** se genera en el teléfono del usuario.

## Qué sabe hacer el backend

Todo corre en el dispositivo del usuario. No hay base central. No se puede consultar entre datos de usuarios distintos: cada consulta ve solo las colecciones propias o las que alguien compartió. Lo que debe verlo todo el mundo (ranking, catálogo, tablón) se publica a propósito en una colección pública.

| Área | Funciones (API de Android) | Para qué |
|---|---|---|
| Datos | `collection(name)`, `save(id, fields)`, `read(id)`, `all()`, `delete(id)` | Abrir una colección y crear, leer, listar y borrar documentos. |
| Consultas | `where(field, operator, value)`, `sortBy(field, descending)`, `limit(n)`, `get()` | Filtrar, ordenar y limitar. Operadores: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza`, `existe`. |
| Tiempo real | `subscribe(listener)`, `Query.subscribe(listener)` | Recibir los documentos ahora y cada vez que cambian (por ti, tus otros dispositivos o quien comparte). |
| Mensajes | `sendMessage(contact, text, onDone)`, `messages()`, `onReceive(listener)` | Enviar y listar mensajes, y avisar cuando llega algo. |
| Archivos | `sendFile(contact, file, name, onDone)` | Enviar una foto, vídeo o documento cifrado. |
| Contactos e invitaciones | `invitation()`, `addContact(name, invitation)`, `contacts()`, `block(contact, blocked)` | Mostrar tu invitación, añadir contactos, listarlos y bloquear. |
| Permisos de compartición | `shareWith(contact, canChange)`, `unshareWith(contact)` | Compartir una colección con lectura o escritura, y retirarla. |
| Sincronización | `syncWith(contact, onDone)` | Sincronizar ahora lo compartido con un contacto. También ocurre solo cada pocos segundos; los dispositivos de la misma cuenta se sincronizan igual. |
| Colecciones públicas | `publicCollection(name)`, `publish(id, fields, onDone)`, `delete(id, onDone)`, `PublicCollection.where/sortBy/get`, `usePublicServer(invitation)` | Publicar lo que todos deben ver y consultarlo por red (hasta 50, o 100 con `limit`). |
| Guía de nombres | `registerName(username)`, `userExists(username)`, `addByName(username)`, `acceptRequests()`, `useNameGuide(invitation)` | Registrar y buscar usuarios por nombre, añadir por nombre y aceptar solicitudes. |
| Copias y guardianes | `recoveryBackup(password)`, `paperCode()`, `protectWithGuardian(username, password)`, `MobileMesh.recover(code, password)`, `MobileMesh.recoverWithGuardian(username, password)`, `MobileMesh.recoverWithPaper(code)` | Copia cifrada de la cuenta, código en papel, protección en un guardián y recuperación en un teléfono nuevo. |
| Relés y supernodos | `useRelay(address)`, `actAsSupernode(enabled)`, `isSupernode()`, `offlineMode(enabled)`, `checkMailboxEvery(minutes)` | Usar un relé propio, ayudar como relé, trabajar sin conexión y ajustar el buzón. |
| Estado de la red | `state(context)`, `myId`, `diagnostics()`, `checkDatabase()` | Ver el estado de la cuenta, el identificador público y un resumen de depuración o salud de la base. |
| Ayudantes | `call(helper, function, data, onDone)` | Llamar a una función de servidor de un ayudante añadido como contacto. |

## Receta de integración de una app nueva (Android)

1. Añade la dependencia y el repositorio de JitPack (ver «Dependencia»).
2. Añade la marca: `apply(from = "../../tools/mobilemesh.gradle.kts")` en `build.gradle.kts`. La primera compilación crea `src/main/assets/mobilemesh.properties`.
3. Elige el segundo plano antes de arrancar: `MobileMesh.backgroundMode(this, BackgroundMode.ALARM)`.
4. Abre la cuenta y una colección:
   ```kotlin
   val mesh = MobileMesh.start(this)
   val notas = mesh.collection("notas")
   ```
5. Guarda y suscríbete:
   ```kotlin
   notas.save("n1", mapOf("texto" to "hola", "hora" to System.currentTimeMillis()))
   val dejarDeEscuchar = notas.sortBy("hora", descending = true).subscribe { docs ->
       docs.forEach { println(it.fields["texto"]) }
   }
   ```
6. Comparte: muestra `mesh.invitation()`, pide la invitación del otro y añade el contacto con `mesh.addContact(nombre, invitacion)`, luego `notas.shareWith(nombre, canChange = true)`.
7. Pide permisos en `onResume()`: `Permissions.request(this)`. Cancela la suscripción en `onDestroy()` llamando a la función que devolvió `subscribe`.

Modelos reales: `examples/notas/src/main/java/org/mobilemesh/notas/NotasActivity.kt` (notas compartidas) y `examples/lote-a/src/main/java/org/pruebalote/a/LoteActivity.kt` (marca, invitación y chat compartido).

## Cómo comprobar que funciona

```sh
cd core && cargo test                 # pruebas del núcleo
cd core && cargo test --test aislamiento   # una app no acepta datos ni invitaciones de otra marca
android/build.sh                      # núcleo Android + Kotlin + app de ejemplo
web/construir.sh                      # núcleo WebAssembly en web/pkg
```

## Errores típicos y qué significan

| Mensaje | Qué significa |
|---|---|
| «la invitación es de otra aplicación» | La invitación se generó con otra marca. Solo se comunican copias con el mismo `appId`. |
| «esta base de datos pertenece a otra aplicación» | La base ya existe con otra marca; no se puede abrir con esta. No borres ni cambies la marca. |
| «falta el identificador de aplicación (appId)» | No hay marca: falta `mobilemesh.properties` en Android o no pasaste `appId` en web. |
| «invitación de una versión anterior» | La invitación es de un protocolo viejo; genera una nueva. |
| «was compiled with an incompatible version of Kotlin» | Falta Kotlin 2.2.0 o superior. |
| Las invitaciones `mobilemesh:2:…` no valen | El protocolo v3 lleva la marca; las v2 ya no se aceptan. |

## Dónde mirar
- `docs/referencia-api.md`: lista completa de funciones (generada del código).
- `docs/mapa.md`: mapa de dónde está cada archivo y para qué sirve (generado del código).
- `docs/sdk.md`: guía de Android · `docs/sdk-web.md`: web · `docs/react-native.md`: React Native.
- `docs/mcp.md`: usar MobileMesh desde una IA por MCP · `docs/protocolo.md`: protocolo.

`docs/referencia-api.md` se regenera con `python3 tools/generar-referencia.py`; no se edita a mano.
