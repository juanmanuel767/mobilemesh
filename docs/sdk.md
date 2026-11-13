# Guía de MobileMesh para programadores (Android)

MobileMesh te da base de datos, cuentas, mensajes, archivos, permisos y sincronización **sin
servidores**: cada usuario guarda sus datos cifrados en su teléfono y los comparte cifrados de
extremo a extremo. No necesitas Firebase ni Supabase.

Ejemplo completo y funcionando: [`examples/notas`](../examples/notas/src/main/java/org/mobilemesh/notas/NotasActivity.kt).

## 0. Instalar (una línea)

```kotlin
// settings.gradle.kts
dependencyResolutionManagement { repositories { google(); mavenCentral(); maven("https://jitpack.io") } }
// build.gradle.kts de tu app
dependencies { implementation("com.github.juanmanuel767:mobilemesh:v0.2.0") }
```

Ya compilado, con el núcleo en Rust dentro. Funciona en móviles reales (`arm64-v8a`) y en el emulador de Android Studio (`x86_64`).

La API está en inglés (`start`, `collection`, `save`, `subscribe`…). Los nombres en español siguen funcionando como alias obsoletos (`@Deprecated`): el código que ya los usaba compila igual, solo verás avisos.

`minSdk` 28. La librería añade sola sus permisos (red, avisos, segundo plano).

## 1. Empezar

```kotlin
val mesh = MobileMesh.start(context, port = 7811) // crea la cuenta la primera vez
```

- Cada app del mismo teléfono debe usar un `port` distinto.
- Si el teléfono está bloqueado lanza `MobileMeshError` (la cuenta solo se abre desbloqueado);
  consúltalo antes con `MobileMesh.state(context)`.
- MobileMesh queda recibiendo en segundo plano (servicio con notificación fija).
- Para que reciba al 100 % con la app cerrada, llama en `onResume` a `Permissions.request(this)`: pide uno
  por uno los avisos, la batería sin restricciones y el inicio automático del fabricante (TECNO,
  Xiaomi, Huawei, Oppo, Vivo, ZTE, Samsung). Solo pregunta lo que falte.

### Cuánto trabaja con la app cerrada

Elige el modo **antes** de `start`. Por defecto es `FULL`, que es como se ha comportado siempre.

```kotlin
MobileMesh.backgroundMode(this, BackgroundMode.ALARM)   // antes de start()
val mesh = MobileMesh.start(this)
```

| Modo | Qué hace | Qué cuesta |
|---|---|---|
| `FULL` | Mensajes al instante, sincroniza con la app cerrada | Notificación fija y tres permisos que Google Play restringe: `FOREGROUND_SERVICE_REMOTE_MESSAGING`, `SCHEDULE_EXACT_ALARM` y `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`. Hay que justificarlos en la ficha de Play |
| `ALARM` | Android despierta la app cada pocos minutos, mira el buzón del relé y avisa | Ningún permiso restringido y ninguna notificación fija. Los avisos llegan con unos minutos de retraso |
| `NONE` | Nada en segundo plano. Leer y escribir datos sigue igual | No recibe nada con la app cerrada |

Si tu app no es de mensajería, `ALARM` es casi siempre lo que quieres: evita la revisión de Play
por los permisos restringidos. Guardar, leer y consultar datos funcionan igual en los tres modos.

En Android 13 y posteriores, para que se vean los avisos hay que pedir `POST_NOTIFICATIONS` en
tiempo de ejecución; `Permissions.request(this)` ya lo hace.

## 2. Datos: colecciones

```kotlin
val orders = mesh.collection("orders")
orders.save("p1", mapOf("product" to "bread", "quantity" to 2, "paid" to false))
orders.read("p1")?.fields?.get("quantity")      // 2
orders.all()                                  // List<Document>
orders.save("p1", mapOf("paid" to true))   // cambia solo ese campo
orders.delete("p1")

val parar = orders.subscribe { docs -> show(docs) } // tiempo real
parar()                                                // dejar de escuchar
```

Consultas: filtrar, ordenar y limitar (también en tiempo real):

```kotlin
orders.where("price", ">", 10).where("paid", "==", false)
    .sortBy("price", descending = true).limit(20).get()
orders.where("product", "contiene", "bread").subscribe { docs -> show(docs) }
```

Operadores: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene` y `empieza` (sin distinguir mayúsculas) y
`existe`. Los números se comparan como números. Como los datos están cifrados, la consulta se hace
en el teléfono sobre la colección: va bien hasta miles de documentos.

- Los valores pueden ser texto, números o booleanos. `null` borra el campo.
- Todo se guarda **al momento en el teléfono**, aunque no haya red, y se sincroniza después.
- Si dos teléfonos cambian **campos distintos**, se conservan los dos cambios; si cambian **el
  mismo**, gana el más reciente.
- Para dinero o inventario usa `mesh.collection("cash", strict = true)`: los cambios simultáneos no
  se mezclan solos y aparecen en `Document.conflicts` hasta que el usuario elija (guardando el
  valor correcto).

## 3. Usuarios y contactos

No hay contraseñas en ningún servidor: la cuenta es una clave protegida en el teléfono.

```kotlin
val myInvitation = mesh.invitation()          // texto para un QR; solo datos públicos
mesh.addContact("Ana", anasInvitation)   // true si era otro teléfono tuyo
mesh.contacts()
mesh.block("Ana")
```

### Tu primer mensaje

Hacen falta **dos dispositivos** (dos móviles, o un móvil y un emulador), cada uno con su cuenta y con red. Los dos ejecutan `MobileMesh.start(context)`; después hay dos caminos para que se conozcan.

Por nombre (sin intercambiar nada a mano):

```kotlin
// Dispositivo A
mesh.registerName("ana")      // aparece en la guía de nombres
// Dispositivo B
mesh.addByName("ana")         // la añade y le avisa
// Dispositivo A: añade a quien le ha pedido hablar (devuelve sus nombres)
mesh.acceptRequests()
// Cualquiera de los dos
mesh.sendMessage("ana", "hola") { r -> r.onSuccess { /* enviado */ } }
```

Con invitación (un QR o texto, en ambos sentidos):

```kotlin
val myInvitation = mesh.invitation()     // cada uno genera la suya y se la pasa al otro
mesh.addContact("Ana", anasInvitation)   // y añade la del otro
mesh.sendMessage("Ana", "hola") { r -> r.onSuccess { /* enviado */ } }
```

El que recibe se entera con `mesh.onReceive { count -> … }` y lee con `mesh.messages()`. `invitation()` y `registerName()` necesitan conexión a una red.

Recuperar la cuenta si se pierde el teléfono:

```kotlin
val codigo = mesh.recoveryBackup("una contraseña larga") // el usuario lo guarda fuera
MobileMesh.recover(context, codigo, "una contraseña larga")  // en el teléfono nuevo
```

### Colecciones públicas: lo que ven todos

Para lo que la app quiere que vean **todos** sus usuarios (un ranking, un catálogo, un tablón). Va a la
pieza pública (la universal o la tuya); todo lo demás sigue privado en cada móvil. Cada documento
lleva su autor y nadie puede cambiar ni borrar lo de otro. Hasta 4 KB por documento y 200 por cuenta
en cada colección; consultas con los mismos filtros, orden y límite (≤100).

```kotlin
val ranking = mesh.publicCollection("ranking")
ranking.publish("ana", mapOf("name" to "Ana", "points" to 120))
ranking.sortBy("points", descending = true).limit(10).get { r ->
    r.onSuccess { top -> top.forEach { println("${it.fields["name"]}: ${it.fields["points"]}") } }
}
```

Tu propia pieza: `mesh <carpeta> publico 0.0.0.0:7956 <ruta pública>` y
`mesh.usePublicServer("<su invitación>")`.

## 4. Compartir datos con permisos

```kotlin
val list = mesh.collection("list")
list.shareWith("Ana", canChange = false) // Ana solo puede leer
list.shareWith("Ana", canChange = true)  // Ana puede leer y cambiar
list.unshareWith("Ana")
```

Para recibir lo que Ana comparte, comparte tú la misma colección con ella. Lo compartido se
sincroniza solo cada pocos segundos (o ya mismo con `list.syncWith("Ana")`). Nadie puede
cambiar lo que solo puede leer ni hacer cambios a nombre de otro.

## 5. Mensajes y archivos

```kotlin
mesh.sendMessage("Ana", "hola") { r -> r.onFailure { /* queda pendiente, se reintenta solo */ } }
mesh.sendFile("Ana", uriDeLaFoto, "foto.jpg")
mesh.messages()
val parar = mesh.onReceive { count -> /* llegó algo nuevo */ }
```

Las operaciones de red trabajan en segundo plano y avisan en el hilo principal.

**Avisos con la app cerrada o la pantalla apagada (sin Google):** si el destino está dormido, el
mensaje queda cifrado en su buzón del relé (`Delivery.inMailbox = true`). Su teléfono despierta cada pocos
minutos, lo recoge, avisa y devuelve el acuse de recibo. No es instantáneo:

```kotlin
mesh.checkMailboxEvery(3) // minutos (3 por defecto); en reposo profundo Android lo espacia a ~10
```

### Avisos al instante sin Google (encendido por defecto)

```kotlin
mesh.instantReception(false) // encendida por defecto; así se apaga
```

Mantiene el sonido abierto en silencio: los fabricantes no congelan una app que suena, así que la
conexión sigue viva con la pantalla apagada y los mensajes llegan al instante (en un TECNO: 1 s con la
pantalla apagada). Con batería por debajo del 15 % (sin cargar) se pausa sola y los avisos llegan
por el despertador; al cargar vuelve.

### Avisos al instante con Firebase (opcional)

Algunos teléfonos (TECNO, ZTE…) congelan las apps y solo dejan despertarlas al instante a Google.
Si lo necesitas, añade la pieza opcional `mobilemesh-avisos-fcm`. Google solo lleva «tienes algo»;
el mensaje sigue cifrado en tu relé.

```kotlin
implementation(project(":mobilemesh-avisos-fcm"))
AvisosFcm.activar(context, FirebaseOptions.Builder()
    .setProjectId("mi-proyecto").setApplicationId("1:123:android:abc")
    .setApiKey("AIza…").setGcmSenderId("123").build())
```

Y en el servidor, el relé con el programa de avisos y la clave de servicio de tu proyecto:

```sh
FCM_CUENTA=/ruta/cuenta-servicio.json mesh-rele 0.0.0.0:7900 --avisos tools/avisar-fcm.py
```

## 6. Funciones de servidor: ayudantes

Para lo que no puede hacer el teléfono (cobrar un pago, mandar un correo, consultar otra API) pon un
**ayudante** en tu servidor: es un MobileMesh sin pantalla que ejecuta tu programa, en el lenguaje
que quieras. La llamada y la respuesta van cifradas de extremo a extremo, y el ayudante sabe con
certeza qué cuenta le llama.

```sh
mesh ayudante-tienda init
mesh ayudante-tienda invitacion mi-servidor.com:7953      # esta invitación la llevan tus apps
mesh ayudante-tienda ayudante 0.0.0.0:7953 ./funciones.sh
```

`funciones.sh` recibe el nombre de la función como argumento y `{"de": cuenta, "datos": {…}}` por
la entrada; lo que escriba por la salida es el resultado, y si termina con error, lo que escriba en
la salida de errores es el error (ejemplo: `tools/ejemplo-ayudante.sh`). Como mucho 30 s por llamada.

```kotlin
mesh.addContact("tienda", helperInvitation)
mesh.call("tienda", "cobrar", mapOf("pedido" to "p1", "total" to 12.5)) { r ->
    r.onSuccess { resultado -> … }.onFailure { error -> … }
}
```

Las llamadas y respuestas no aparecen en `messages()` ni hacen sonar avisos. Medido: respuesta en
0,4 s desde un TECNO por wifi.

## 7. Redes distintas: tu propio relé

Entre wifi y datos móviles hace falta un relé (no puede leer nada):

```sh
mesh-rele 0.0.0.0:7900   # en un equipo con dirección pública
```

```kotlin
mesh.useRelay("mi-servidor.com:7900") // se aplica al reiniciar la app
```

### Supernodos: los usuarios ayudan (opcional)

Encendido por defecto, el teléfono hace de relé ligero para otros usuarios **solo en wifi y con
batería ≥15 % (o cargando)**. Si deja de cumplirse, para solo y la app sigue igual. No puede leer
nada de lo que pasa por él. Así, cuantos más usuarios, más capacidad, sin depender de un servidor.

```kotlin
mesh.actAsSupernode(false)  // encendido por defecto; así se apaga (p. ej. si el usuario no quiere)
mesh.isSupernode()            // ¿lo está siendo ahora?
```

Las apps eligen solas un supernodo (y el relé del proyecto queda de respaldo). Como usa un poco del
internet del usuario, dilo en la política de privacidad de tu app y ofrécele apagarlo.

## 8. Errores y diagnóstico

- Todos los errores son `MobileMeshError` con un mensaje en español listo para mostrar.
- `mesh.diagnostics()` devuelve un resumen sin datos privados: cuenta, dispositivo, relé, contactos,
  pendientes, cambios sin confirmar, copias.
- `mesh.offlineMode(true)` simula no tener red.

## Lo que todavía no hace

- Avisos instantáneos con la pantalla apagada (llegan en el siguiente despertar, ver sección 5).
- Archivos por el buzón: esperan a la entrega directa.
- iPhone y navegador (el núcleo en Rust ya está preparado para ello).
- Buscar entre datos de distintos usuarios (no hay ningún sitio con los datos de todos: es la idea).

## Compilar desde el código fuente

Solo si quieres modificar el SDK: clona el repositorio, compila el núcleo con `android/build.sh`, incluye el módulo en `settings.gradle.kts` (`include(":mobilemesh-sdk")`) y en tu app:

```kotlin
dependencies {
    implementation(project(":mobilemesh-sdk"))
}
```

Para usar el SDK tal cual, la instalación de JitPack del principio es suficiente.
