<!-- GENERADO por tools/generar-mapa.py. No editar a mano. -->
# Mapa del proyecto

> **Archivo GENERADO** a partir del código. No se edita a mano: se regenera con
> `python3 tools/generar-mapa.py`.

Dónde está cada cosa y para qué sirve. El propósito sale del comentario de cabecera de cada archivo;
si no lo tiene aparece como _(sin descripción en el archivo)_ y se lista al final.

**Archivos mapeados:** 109 · **sin descripción:** 14

## Núcleo (core/src)

El corazón en Rust: almacenamiento, sincronización, red, cifrado, identidad, relé y la interfaz FFI que usan los SDK.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `core/src/access.rs` | Permisos y control de acceso (Fase 10). | 149 |
| `core/src/directorio.rs` | Directorio (pieza aparte): lista **firmada** de relés y guardianes universales que las apps descargan solas de su relé al arrancar. | 93 |
| `core/src/e2e.rs` | Cifrado de extremo a extremo por mensaje: Double Ratchet (Olm de vodozemac, biblioteca auditada). | 119 |
| `core/src/ffi.rs` | Puerta de entrada para Android (Kotlin) y, más adelante, iPhone (Swift). | 1192 |
| `core/src/files.rs` | Archivos (fotos, vídeos, documentos): se cifran por trozos con una clave propia de cada archivo. | 220 |
| `core/src/funciones.rs` | Funciones de servidor sin servidor central: un **ayudante** es un MobileMesh sin pantalla que corre donde quiera el programador (su servidor, una Raspberry…). | 121 |
| `core/src/guardian.rs` | Guardián (pieza aparte, va junto al relé): un MobileMesh sin pantalla que guarda, para cualquiera, lo necesario para recuperarse tras perder el teléfono: | 183 |
| `core/src/identity.rs` | Identidad: una **cuenta** (clave de firma Ed25519) y **este dispositivo** (clave X25519 de Noise). | 309 |
| `core/src/lib.rs` | Núcleo de MobileMesh: identidad, base de datos local y conexión directa cifrada. | 818 |
| `core/src/net.rs` | Conexiones (pieza aparte). | 54 |
| `core/src/nombres.rs` | Guía de nombres (pieza aparte, va junto al relé): un MobileMesh sin pantalla que hace de «agenda pública», para que las apps funcionen como cualquier base de datos con usuarios: | 211 |
| `core/src/publico.rs` | Colecciones públicas (pieza aparte, va junto al relé): lo único de una app que se guarda fuera de los dispositivos, porque la app lo publica **a propósito** para que todos lo consulten (un ranking, un catálogo, un tablón). | 183 |
| `core/src/recovery.rs` | Copia de recuperación: la clave de la **cuenta** cifrada con una contraseña que elige el usuario. | 184 |
| `core/src/relay.rs` | Relé (Fase 9): une dos conexiones cuando los dispositivos no pueden conectarse directamente (redes distintas, datos móviles, routers). | 285 |
| `core/src/replica.rs` | Copias de seguridad cifradas guardadas en otros dispositivos (Fase 8). | 311 |
| `core/src/store.rs` | Base de datos local (SQLite): contactos, mensajes con su estado y estado del cifrado. | 1250 |
| `core/src/supernodo.rs` | Supernodo: el móvil de un usuario hace de relé ligero para otros mientras está en wifi y con batería (lo decide la app). | 233 |
| `core/src/sync.rs` | Datos sincronizados entre los dispositivos de una cuenta (Fase 7). | 425 |
| `core/src/transport.rs` | Conexión directa cifrada entre dos dispositivos (TCP + Noise XX). | 377 |
| `core/src/upnp.rs` | Abrir un puerto en el router de casa por UPnP (pieza aparte). | 139 |
| `core/src/web.rs` | Versión para navegador (pieza aparte, solo WebAssembly). | 285 |

## Pruebas del núcleo (core/tests)

Pruebas de integración del núcleo: durabilidad, ataques, red, migración y medidas de velocidad.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `core/tests/ataques.rs` | Fase 13: ataques simulados. | 183 |
| `core/tests/durabilidad.rs` | Lo que se le exige a una base de datos: que no se corrompa si el proceso muere escribiendo, que dos escritores a la vez no se pisen y que un archivo alterado se note en vez de leerse como si fuera bueno. | 329 |
| `core/tests/invitacion.rs` | Invitaciones que la propia app genera y que otra app debe aceptar. | 52 |
| `core/tests/medidas.rs` | Fase 13: mediciones de coste (ejecutar con `cargo test --release --test medidas -- --nocapture`). | 125 |
| `core/tests/migracion.rs` | Pasar una base de datos de una versión anterior (sin cifrar) a la base cifrada, sin perder ni un dato. | 142 |
| `core/tests/red.rs` | Pruebas de la Fase 4/5 con dos o tres "teléfonos" simulados en el mismo equipo. | 1621 |
| `core/tests/velocidad.rs` | Mediciones de velocidad reproducibles (no fallan: imprimen). | 90 |

## SDK Android

La librería Kotlin para apps Android y sus piezas auxiliares (avisos, app de demostración).

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `android/app/src/main/java/org/mobilemesh/demo/MainActivity.kt` | _(sin descripción en el archivo)_ | 739 |
| `android/mobilemesh-avisos-fcm/src/main/java/org/mobilemesh/avisosfcm/AvisosFcm.kt` | Avisos al instante por Firebase (opcional). | 54 |
| `android/mobilemesh-sdk/src/main/java/org/mobilemesh/sdk/KeystoreGuard.kt` | Cifra la identidad con una clave AES que vive en Android Keystore: esa clave no se puede sacar del teléfono, así que copiar el archivo de identidad a otro equipo no sirve de nada. | 49 |
| `android/mobilemesh-sdk/src/main/java/org/mobilemesh/sdk/MeshService.kt` | _(sin descripción en el archivo)_ | 499 |
| `android/mobilemesh-sdk/src/main/java/org/mobilemesh/sdk/MobileMesh.kt` | _(sin descripción en el archivo)_ | 1076 |
| `android/mobilemesh-sdk/src/main/java/org/mobilemesh/sdk/Permissions.kt` | Pide al usuario, uno por uno, los permisos para que MobileMesh funcione al 100 % (recibir con la app cerrada y la pantalla apagada): avisos, batería sin restricciones e inicio automático del fabricante. | 110 |

## Puente React Native

El módulo que expone MobileMesh a apps React Native, con su parte nativa de Android.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `react-native.config.js` | Para que React Native encuentre solo el puente de Android (autolinking). | 13 |
| `react-native/android/src/main/java/org/mobilemesh/reactnative/MobileMeshModule.kt` | Puente de MobileMesh para React Native: expone el SDK de Android a JavaScript. | 307 |
| `react-native/android/src/main/java/org/mobilemesh/reactnative/MobileMeshPackage.kt` | _(sin descripción en el archivo)_ | 10 |
| `react-native/android/src/main/java/org/mobilemesh/reactnative/VolverActivity.kt` | Recibe «<id de la app>://pago» (regreso de una pasarela de pago) y trae la app al frente. | 16 |
| `react-native/index.js` | MobileMesh para React Native (Android): base de datos, mensajes, archivos y avisos sin servidores. | 187 |

## SDK web

El SDK para navegador: envoltorio JavaScript sobre el núcleo compilado a WASM.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `core/js/conexion.js` | Conexiones del núcleo en el navegador (lado del núcleo, dentro de su trabajador). | 56 |
| `web/mobilemesh.js` | SDK web de MobileMesh. | 188 |
| `web/nucleo-worker.js` | Trabajador con el núcleo de MobileMesh (WebAssembly). | 58 |
| `web/red.js` | Red de MobileMesh en el navegador (lado de la página). | 61 |
| `web/servir.py` | Sirve esta carpeta con las cabeceras que necesita el SDK web (memoria compartida). | 14 |

## Relé y servidor

Los programas que hacen de relé y de servidor, incluida la app Android que convierte un móvil en relé.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `android/rele-movil/src/main/java/org/mobilemesh/rele/Rele.kt` | _(sin descripción en el archivo)_ | 292 |
| `servidor/src/bin/mesh-rele.rs` | `mesh-rele`: relé de MobileMesh que cualquiera puede ejecutar (en casa, en un servidor…). | 84 |
| `servidor/src/lib.rs` | Programas de servidor de MobileMesh (licencia AGPL-3.0): el relé, el puente para navegadores (WebSocket) y el candado (wss). | 10 |
| `servidor/src/rele.rs` | El relé en sí (pieza de servidor, licencia AGPL-3.0): buzones, empalme de conexiones, cartas, avisos, límites y velocidad. | 602 |
| `servidor/src/tls.rs` | Candado (wss://) para el puente web del relé (pieza aparte). | 99 |
| `servidor/src/websocket.rs` | Puente WebSocket del relé (pieza aparte): los navegadores no pueden abrir conexiones TCP, solo WebSocket. | 227 |

## Línea de órdenes y herramientas (core/src/bin, tools/)

La CLI `mesh`, el servidor MCP y los scripts de operación del repositorio.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `core/src/bin/mesh-directorio.rs` | `mesh-directorio`: crea la clave del proyecto y firma el directorio de relés y guardianes que descargan las apps. | 46 |
| `core/src/bin/mesh-mcp.rs` | `mesh-mcp`: servidor MCP (Model Context Protocol) para que cualquier IA (Claude, ChatGPT, Codex…) use MobileMesh: leer la guía del SDK, crear nodos de prueba, guardar y consultar datos, añadir contactos, dar permisos y comprobar la red. | 173 |
| `core/src/main.rs` | `mesh`: herramienta de pruebas para simular dispositivos desde la terminal. | 468 |
| `tools/abrir-puerto-upnp.sh` | Pide al router de casa (por UPnP, sin contraseña) que abra el puerto del relé hacia el móvil que lo ejecuta. | 35 |
| `tools/avisar-fcm.py` | Aviso opcional por Firebase (FCM) para el relé de MobileMesh. | 83 |
| `tools/ejemplo-ayudante.sh` | Ejemplo de funciones del ayudante: $1 = función, entrada = {"de": cuenta, "datos": {...}} | 8 |
| `tools/generar-mapa.py` | Genera docs/mapa.md: dónde está cada archivo y para qué sirve (sin dependencias externas). | 383 |
| `tools/generar-referencia.py` | Genera docs/referencia-api.md a partir del código (sin dependencias externas). | 571 |
| `tools/instalar-rele.sh` | Instala el relé de MobileMesh en un servidor Linux (Oracle Cloud, cualquier VPS o un PC de casa) y lo deja arrancando solo, también tras reiniciar. | 49 |
| `tools/publicar-directorio.sh` | Firma la lista de relés y guardianes y la deja en el relé del móvil (app «Relé MobileMesh»). | 15 |
| `tools/renovar-certificado.sh` | Renueva (si toca) el certificado gratis de Let's Encrypt del relé y lo deja en el móvil del relé. | 13 |

## Ejemplos

Apps y ayudantes de ejemplo que muestran cómo usar MobileMesh.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `examples/ayudante-pagos/pagos.py` | Ayudante de pagos para MobileMesh (funciones de servidor), con cualquier pasarela. | 100 |
| `examples/ayudante-pagos/pasarelas/__init__.py` | _(sin descripción en el archivo)_ | 0 |
| `examples/ayudante-pagos/pasarelas/mercadopago.py` | Mercado Pago (Checkout Pro: Latinoamérica). | 34 |
| `examples/ayudante-pagos/pasarelas/stripe.py` | Stripe (Checkout: tarjetas, Apple Pay y más, en muchos países). | 36 |
| `examples/ayudante-pagos/pasarelas/wompi.py` | Wompi (Colombia: tarjeta, Nequi, PSE, Bancolombia, Daviplata…). | 36 |
| `examples/ayudante-turn/turn.py` | Ayudante de TURN para MobileMesh: credenciales temporales para las llamadas. | 128 |
| `examples/notas/src/main/java/org/mobilemesh/notas/NotasActivity.kt` | Ejemplo de app hecha con MobileMesh en lugar de Firebase o Supabase: notas que se guardan en el teléfono y se comparten con otra persona, sin servidor. | 110 |

## Documentación

Guías, protocolo y decisiones de diseño en Markdown.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `docs/comparacion.md` | MobileMesh frente a Firebase, Supabase y SQLite (comparación honesta): No hay una herramienta mejor para todo. | 48 |
| `docs/decisiones/0001-tecnologias.md` | Decisión 0001: tecnologías del núcleo: Fecha: 12-10-2026 · Estado: aceptada | 20 |
| `docs/decisiones/0002-identidad-y-recuperacion.md` | Decisión 0002: protección de la identidad y recuperación (Fase 2): Fecha: 12-10-2026 · Estado: aceptada | 35 |
| `docs/decisiones/0003-cifrado-de-extremo-a-extremo.md` | Decisión 0003: cifrado de extremo a extremo (Fase 5): Fecha: 12-10-2026 · Estado: aceptada | 29 |
| `docs/decisiones/0004-archivos.md` | Decisión 0004: fotos, vídeos y documentos (Fase 6): Fecha: 12-10-2026 · Estado: aceptada | 32 |
| `docs/decisiones/0005-sincronizacion.md` | Decisión 0005: modo sin conexión y sincronización (Fase 7): Fecha: 12-10-2026 · Estado: aceptada | 31 |
| `docs/decisiones/0006-copias-en-otros-dispositivos.md` | Decisión 0006: copias cifradas en otros dispositivos (Fase 8): Fecha: 12-10-2026 · Estado: aceptada | 32 |
| `docs/decisiones/0007-rele-y-metadatos.md` | Decisión 0007: conexión entre redes distintas, relé y metadatos (Fase 9): Fecha: 12-10-2026 · Estado: aceptada | 47 |
| `docs/decisiones/0008-permisos.md` | Decisión 0008: permisos y control de acceso (Fase 10): Fecha: 12-10-2026 · Estado: aceptada | 30 |
| `docs/decisiones/0009-avisos-y-segundo-plano.md` | Decisión 0009: recibir con la app cerrada y avisos (Fase 11): Fecha: 12-10-2026 · Estado: en curso | 42 |
| `docs/decisiones/0010-sdk.md` | Decisión 0010: SDK para desarrolladores (Fase 12): Fecha: 13-10-2026 · Estado: aceptada | 35 |
| `docs/decisiones/0011-auditoria.md` | Informe de la Fase 13: seguridad, auditoría y preparación del lanzamiento: Fecha: 13-10-2026 | 59 |
| `docs/decisiones/0012-guardian.md` | Decisión 0012: guardián para recuperar cuenta y datos: Fecha: 15-10-2026 · Estado: aceptada | 28 |
| `docs/decisiones/0013-directorio.md` | Decisión 0013: directorio firmado de relés y guardianes: Fecha: 15-10-2026 · Estado: aceptada | 30 |
| `docs/decisiones/0014-auditoria-piezas-nuevas.md` | Decisión 0014: auditoría de las piezas nuevas: Fecha: 15-10-2026 · Estado: aceptada | 34 |
| `docs/decisiones/0015-guia-de-nombres.md` | Decisión 0015: guía de nombres y conexión en carrera: Fecha: 16-10-2026 · Estado: aceptada | 31 |
| `docs/dependencias.md` | Dependencias y licencias: Revisado el 13-10-2026 con cargo metadata y cargo audit. | 31 |
| `docs/donar-un-rele.md` | Cómo donar un relé (y un guardián): Un relé conecta teléfonos de redes distintas; un guardián guarda copias cifradas para recuperar cuentas. | 66 |
| `docs/fase-0.md` | Fase 0: reglas, límites y amenazas: 1. | 55 |
| `docs/mcp.md` | MobileMesh para IA (servidor MCP): mesh-mcp deja que cualquier IA compatible con MCP (Claude, ChatGPT, Codex…) use MobileMesh: leer la guía del SDK, crear nodos de prueba, guardar y consultar datos, añadir contactos, dar permisos y comprobar la red. | 28 |
| `docs/protocolo.md` | Protocolo de MobileMesh (versión 2): Descripción para quien quiera revisar el protocolo o hacer una implementación compatible. | 98 |
| `docs/react-native.md` | Guía de MobileMesh para React Native (Android): Base de datos, cuentas por nombre de usuario, mensajes, archivos y avisos, sin servidores de datos. | 75 |
| `docs/referencia-api.md` | Referencia de la API de MobileMesh: > Archivo GENERADO. | 302 |
| `docs/rendimiento.md` | Rendimiento de MobileMesh (medido): Todo medido con el núcleo de verdad: cifrado, SQLite en disco, Noise y ratchet. | 59 |
| `docs/sdk-web.md` | Guía de MobileMesh para la web: El mismo núcleo que en Android, compilado a WebAssembly. | 139 |
| `docs/sdk.md` | Guía de MobileMesh para programadores (Android): MobileMesh te da base de datos, cuentas, mensajes, archivos, permisos y sincronización sin servidores: cada usuario guarda sus datos cifrados en su teléfono y los comparte cifrados de extremo a extremo. | 319 |

## Scripts de compilación

Configuración de Gradle, Cargo y publicación que compila y empaqueta cada pieza.

| Archivo | Para qué sirve | Líneas |
|---|---|---:|
| `android/app/build.gradle.kts` | _(sin descripción en el archivo)_ | 30 |
| `android/build.sh` | Compila el núcleo Rust para el teléfono, genera el código Kotlin y construye la app. | 43 |
| `android/mobilemesh-avisos-fcm/build.gradle.kts` | Pieza OPCIONAL: avisos al instante por Firebase (Google). | 19 |
| `android/mobilemesh-sdk/build.gradle.kts` | _(sin descripción en el archivo)_ | 27 |
| `android/rele-movil/build.gradle.kts` | _(sin descripción en el archivo)_ | 26 |
| `android/rele-movil/construir.sh` | Licencia de esta app: AGPL-3.0 (ver LICENSE en esta carpeta). | 21 |
| `android/settings.gradle.kts` | _(sin descripción en el archivo)_ | 30 |
| `core/.cargo/config.toml` | Navegador: getrandom 0.3 usa el generador del navegador (crypto.getRandomValues). | 3 |
| `core/Cargo.toml` | _(sin descripción en el archivo)_ | 60 |
| `core/uniffi-bindgen.rs` | _(sin descripción en el archivo)_ | 3 |
| `examples/notas/build.gradle.kts` | _(sin descripción en el archivo)_ | 26 |
| `jitpack.yml` | JitPack sirve la librería de Android ya compilada (el núcleo en Rust no se puede compilar allí): descarga el .aar publicado en la versión de GitHub y lo instala con su descripción (pom.xml). repositories { maven("https://jitpack.io") } dep… | 11 |
| `react-native/android/build.gradle` | Puente de MobileMesh para React Native. | 24 |
| `servidor/Cargo.toml` | _(sin descripción en el archivo)_ | 17 |
| `web/construir.sh` | Compila el núcleo para navegador (WebAssembly) y genera web/pkg. | 30 |

## Archivos sin descripción

Deuda de documentación: ningún comentario de cabecera que explique su propósito.

- `android/app/build.gradle.kts` (Scripts de compilación)
- `android/app/src/main/java/org/mobilemesh/demo/MainActivity.kt` (SDK Android)
- `android/mobilemesh-sdk/build.gradle.kts` (Scripts de compilación)
- `android/mobilemesh-sdk/src/main/java/org/mobilemesh/sdk/MeshService.kt` (SDK Android)
- `android/mobilemesh-sdk/src/main/java/org/mobilemesh/sdk/MobileMesh.kt` (SDK Android)
- `android/rele-movil/build.gradle.kts` (Scripts de compilación)
- `android/rele-movil/src/main/java/org/mobilemesh/rele/Rele.kt` (Relé y servidor)
- `android/settings.gradle.kts` (Scripts de compilación)
- `core/Cargo.toml` (Scripts de compilación)
- `core/uniffi-bindgen.rs` (Scripts de compilación)
- `examples/ayudante-pagos/pasarelas/__init__.py` (Ejemplos)
- `examples/notas/build.gradle.kts` (Scripts de compilación)
- `react-native/android/src/main/java/org/mobilemesh/reactnative/MobileMeshPackage.kt` (Puente React Native)
- `servidor/Cargo.toml` (Scripts de compilación)
