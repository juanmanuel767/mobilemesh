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
dependencies { implementation("com.github.juanmanuel767:mobilemesh:v0.1.2") }
```

Ya compilado, con el núcleo en Rust dentro (teléfonos de 64 bits, `arm64-v8a`). Para compilarlo tú: `android/build.sh`.

## 1. Añadir la librería

En `settings.gradle.kts` incluye el módulo y en tu app:

```kotlin
dependencies {
    implementation(project(":mobilemesh-sdk"))
}
```

`minSdk` 28. La librería añade sola sus permisos (red, avisos, segundo plano).

## 2. Empezar

```kotlin
val mesh = MobileMesh.iniciar(context, puerto = 7811) // crea la cuenta la primera vez
```

- Cada app del mismo teléfono debe usar un `puerto` distinto.
- Si el teléfono está bloqueado lanza `MobileMeshError` (la cuenta solo se abre desbloqueado);
  consúltalo antes con `MobileMesh.estado(context)`.
- MobileMesh queda recibiendo en segundo plano (servicio con notificación fija).
- Para que reciba al 100 % con la app cerrada, llama en `onResume` a `Permisos.pedir(this)`: pide uno
  por uno los avisos, la batería sin restricciones y el inicio automático del fabricante (TECNO,
  Xiaomi, Huawei, Oppo, Vivo, ZTE, Samsung). Solo pregunta lo que falte.

## 3. Datos: colecciones

```kotlin
val pedidos = mesh.coleccion("pedidos")
pedidos.guardar("p1", mapOf("producto" to "pan", "cantidad" to 2, "pagado" to false))
pedidos.leer("p1")?.campos?.get("cantidad")      // 2
pedidos.todos()                                  // List<Documento>
pedidos.guardar("p1", mapOf("pagado" to true))   // cambia solo ese campo
pedidos.borrar("p1")

val parar = pedidos.escuchar { docs -> mostrar(docs) } // tiempo real
parar()                                                // dejar de escuchar
```

Consultas: filtrar, ordenar y limitar (también en tiempo real):

```kotlin
pedidos.donde("precio", ">", 10).donde("pagado", "==", false)
    .ordenarPor("precio", descendente = true).limite(20).obtener()
pedidos.donde("producto", "contiene", "pan").escuchar { docs -> mostrar(docs) }
```

Operadores: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene` y `empieza` (sin distinguir mayúsculas) y
`existe`. Los números se comparan como números. Como los datos están cifrados, la consulta se hace
en el teléfono sobre la colección: va bien hasta miles de documentos.

- Los valores pueden ser texto, números o booleanos. `null` borra el campo.
- Todo se guarda **al momento en el teléfono**, aunque no haya red, y se sincroniza después.
- Si dos teléfonos cambian **campos distintos**, se conservan los dos cambios; si cambian **el
  mismo**, gana el más reciente.
- Para dinero o inventario usa `mesh.coleccion("caja", estricta = true)`: los cambios simultáneos no
  se mezclan solos y aparecen en `Documento.conflictos` hasta que el usuario elija (guardando el
  valor correcto).

## 4. Usuarios y contactos

No hay contraseñas en ningún servidor: la cuenta es una clave protegida en el teléfono.

```kotlin
val miInvitacion = mesh.invitacion()          // texto para un QR; solo datos públicos
mesh.anadirContacto("Ana", invitacionDeAna)   // true si era otro teléfono tuyo
mesh.contactos()
mesh.bloquear("Ana")
```

Recuperar la cuenta si se pierde el teléfono:

```kotlin
val codigo = mesh.copiaDeRecuperacion("una contraseña larga") // el usuario lo guarda fuera
MobileMesh.recuperar(context, codigo, "una contraseña larga")  // en el teléfono nuevo
```

## 5. Compartir datos con permisos

```kotlin
val lista = mesh.coleccion("lista")
lista.compartirCon("Ana", puedeCambiar = false) // Ana solo puede leer
lista.compartirCon("Ana", puedeCambiar = true)  // Ana puede leer y cambiar
lista.dejarDeCompartirCon("Ana")
```

Para recibir lo que Ana comparte, comparte tú la misma colección con ella. Lo compartido se
sincroniza solo cada pocos segundos (o ya mismo con `lista.sincronizarCon("Ana")`). Nadie puede
cambiar lo que solo puede leer ni hacer cambios a nombre de otro.

## 6. Mensajes y archivos

```kotlin
mesh.enviarMensaje("Ana", "hola") { r -> r.onFailure { /* queda pendiente, se reintenta solo */ } }
mesh.enviarArchivo("Ana", uriDeLaFoto, "foto.jpg")
mesh.mensajes()
val parar = mesh.alRecibir { cuantos -> /* llegó algo nuevo */ }
```

Las operaciones de red trabajan en segundo plano y avisan en el hilo principal.

**Avisos con la app cerrada o la pantalla apagada (sin Google):** si el destino está dormido, el
mensaje queda cifrado en su buzón del relé (`Envio.enBuzon = true`). Su teléfono despierta cada pocos
minutos, lo recoge, avisa y devuelve el acuse de recibo. No es instantáneo:

```kotlin
mesh.mirarBuzonCada(3) // minutos (3 por defecto); en reposo profundo Android lo espacia a ~10
```

### Recepción instantánea sin Google (opcional)

```kotlin
mesh.recepcionInstantanea(true) // mejor que lo active el usuario: gasta más batería
```

Mantiene el sonido abierto en silencio: los fabricantes no congelan una app que suena, así que la
conexión sigue viva con la pantalla apagada (en un TECNO: mensaje entregado en 1 s con la pantalla
apagada; sin esta opción, ni el despertador ni los demás permisos lo consiguieron). A cambio, el chip
de sonido no se duerme.

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

## 7. Funciones de servidor: ayudantes

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
mesh.anadirContacto("tienda", invitacionDelAyudante)
mesh.llamar("tienda", "cobrar", mapOf("pedido" to "p1", "total" to 12.5)) { r ->
    r.onSuccess { resultado -> … }.onFailure { error -> … }
}
```

Las llamadas y respuestas no aparecen en `mensajes()` ni hacen sonar avisos. Medido: respuesta en
0,4 s desde un TECNO por wifi.

## 8. Redes distintas: tu propio relé

Entre wifi y datos móviles hace falta un relé (no puede leer nada):

```sh
mesh-rele 0.0.0.0:7900   # en un equipo con dirección pública
```

```kotlin
mesh.usarRele("mi-servidor.com:7900") // se aplica al reiniciar la app
```

## 9. Errores y diagnóstico

- Todos los errores son `MobileMeshError` con un mensaje en español listo para mostrar.
- `mesh.diagnostico()` devuelve un resumen sin datos privados: cuenta, dispositivo, relé, contactos,
  pendientes, cambios sin confirmar, copias.
- `mesh.modoSinConexion(true)` simula no tener red.

## Lo que todavía no hace

- Avisos instantáneos con la pantalla apagada (llegan en el siguiente despertar, ver sección 6).
- Archivos por el buzón: esperan a la entrega directa.
- iPhone y navegador (el núcleo en Rust ya está preparado para ello).
- Buscar entre datos de distintos usuarios (no hay ningún sitio con los datos de todos: es la idea).
