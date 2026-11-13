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

## Dónde mirar
- `docs/referencia-api.md`: lista completa de funciones (generada del código).
- `docs/mapa.md`: mapa de dónde está cada archivo y para qué sirve (generado del código).
- `docs/sdk.md`: guía de Android · `docs/sdk-web.md`: web · `docs/react-native.md`: React Native.
- `docs/mcp.md`: usar MobileMesh desde una IA por MCP · `docs/protocolo.md`: protocolo.

`docs/referencia-api.md` se regenera con `python3 tools/generar-referencia.py`; no se edita a mano.
