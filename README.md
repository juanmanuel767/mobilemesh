<p align="center"><img src="docs/img/logo.png" alt="MobileMesh: base de datos local en tus dispositivos" width="320"></p>

# MobileMesh

<p align="center"><b>🔒 La base de datos que NO vive en la nube.</b><br>Tus datos se quedan en tus dispositivos. Alternativa gratis a Firebase y Supabase.</p>

<p align="center">
  <a href="https://github.com/juanmanuel767/mobilemesh/releases/latest"><img src="https://img.shields.io/github/v/release/juanmanuel767/mobilemesh?label=versi%C3%B3n" alt="versión"></a>
  <a href="https://jitpack.io/#juanmanuel767/mobilemesh"><img src="https://jitpack.io/v/juanmanuel767/mobilemesh.svg" alt="JitPack"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/licencia-MIT%20%2F%20AGPL-blue" alt="licencia"></a>
  <a href="https://github.com/juanmanuel767/mobilemesh/stargazers"><img src="https://img.shields.io/github/stars/juanmanuel767/mobilemesh?style=social" alt="estrellas"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/n%C3%BAcleo-Rust-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Android-Kotlin-3DDC84?logo=android&logoColor=white" alt="Android">
  <img src="https://img.shields.io/badge/React%20Native-listo-61DAFB?logo=react&logoColor=black" alt="React Native">
  <img src="https://img.shields.io/badge/Web-WebAssembly-654FF0?logo=webassembly&logoColor=white" alt="Web">
  <img src="https://img.shields.io/badge/cifrado-de%20extremo%20a%20extremo-success" alt="cifrado">
</p>

<p align="center">
  <a href="#-empieza-en-1-minuto"><b>🚀 Empezar</b></a> ·
  <a href="#-novedad-supernodos"><b>🌐 Supernodos</b></a> ·
  <a href="#%EF%B8%8F-cómo-funciona"><b>⚙️ Cómo funciona</b></a> ·
  <a href="docs/comparacion.md"><b>📊 vs Firebase</b></a>
</p>

¿Cuántas veces has leído *«se filtraron los datos de millones de usuarios»*? Pasa porque todo se guarda en un mismo servidor en la nube: un solo fallo y se lo llevan todo. MobileMesh funciona al revés:

| | |
|---|---|
| ☁️❌ **Nada vive en la nube** | Los datos de tu app se guardan cifrados en los dispositivos de tus usuarios: móvil, tablet o navegador. |
| 🛡️ **No hay un servidor central que hackear** | No existe una gran base de datos con la información de todos. Para robar los datos de un usuario habría que robarle el móvil… y aun así están cifrados. |
| 🔐 **Cifrado de extremo a extremo, como Signal** | Ni el creador, ni los relés que conectan los móviles, ni nadie en medio puede leer nada. |
| 👤 **Usuarios por nombre** | Registro con usuario y contraseña, sin correo ni teléfono. Se añade a otros por su nombre. |
| 📶 **Funciona sin internet** | La app sigue funcionando sin conexión y se sincroniza sola al volver. |
| 📱 **Si pierdes el móvil, no pierdes nada** | Copia cifrada automática: se recupera con usuario y contraseña, o con un código en papel. |
| 💳 **Pagos** | Cobra con Wompi, Mercado Pago, Stripe o cualquier pasarela en una línea; las claves nunca van dentro de la app. |
| 💸 **Gratis de verdad** | Sin facturas por usuarios, lecturas o almacenamiento: los datos no ocupan servidores. |
| 🏆 **Colecciones públicas** | Lo que la app quiere que vean todos (rankings, catálogos, tablones) en una línea; el resto sigue privado. |
| 🌐 **Crece con sus usuarios** | Los móviles que lo permiten (en wifi y con batería) ayudan a conectar a los demás: cuantos más usuarios, más capacidad. |
| 🧑‍💻 **Para programadores** | SDK para Android, React Native y web, y servidor MCP para que una IA cree apps con él. |

> 💡 Es un proyecto joven. Si te gusta, pruébalo, déjale una ⭐ y cuéntanos qué mejorarías.

📊 **Sin promesas vacías:** [comparación honesta con Firebase, Supabase y SQLite](docs/comparacion.md) (también en qué *no* gana) · [rendimiento medido](docs/rendimiento.md) (repetible con un comando).

## 🌐 Novedad: supernodos

En la mayoría de bases de datos, cuantos más usuarios tienes, más pagas de servidor. En MobileMesh pasa al revés: **los propios usuarios suman capacidad.**

```mermaid
flowchart LR
    A["📱 Ana<br/>(datos móviles)"] -->|cifrado| S["🏠 Supernodo<br/>móvil de Pedro en wifi"]
    S -->|cifrado| B["📱 Juan<br/>(datos móviles)"]
    A -. "respaldo" .-> R["🛰️ Relé del proyecto"]
    R -.-> B
    D["🧭 Director"] -- "«usa a Pedro»" --> A
    S -- "«puedo ayudar»" --> D
```

- Viene **encendido**: un móvil hace de **supernodo** solo cuando está **en wifi** y tiene **batería ≥15 %** (o carga). Si deja de cumplirse, para solo: su app sigue igual.
- **No puede leer nada** de lo que pasa por él: todo va cifrado de punta a punta.
- Si un supernodo se va, las apps pasan al relé del proyecto y eligen otro solas.

```kotlin
mesh.hacerDeSupernodo(false)  // encendido por defecto; así se apaga si el usuario no quiere
```

✅ Probado en móviles reales: un mensaje entre dos móviles con datos móviles pasó por un supernodo en casa, sin tocar el relé del proyecto.

## 🚀 Empieza en 1 minuto

### React Native (Android)

```sh
npm install github:juanmanuel767/mobilemesh#v0.1.10
```

```js
import MobileMesh from "react-native-mobilemesh";

await MobileMesh.iniciar();
await MobileMesh.registrar("ana", "contraseña-larga");   // usuario, sin correo ni teléfono
await MobileMesh.anadirPorNombre("juan");                // su app te añade sola
await MobileMesh.enviar("juan", "hola");                 // chat cifrado en tiempo real
MobileMesh.alRecibir(async () => console.log(await MobileMesh.mensajes()));
await MobileMesh.guardar("tareas", "t1", { titulo: "Comprar pan" });   // base de datos
const { estado } = await MobileMesh.pagar("cafe");                      // cobrar (Wompi, Mercado Pago, Stripe…)
```

En `android/build.gradle`: `minSdkVersion = 28`. Archivos, avisos, recuperar la cuenta y más: [guía de React Native](docs/react-native.md).

### Android (Kotlin)

En `settings.gradle.kts` y en el `build.gradle.kts` de tu app:

```kotlin
dependencyResolutionManagement { repositories { google(); mavenCentral(); maven("https://jitpack.io") } }

dependencies { implementation("com.github.juanmanuel767:mobilemesh:v0.1.10") }
```

```kotlin
val mesh = MobileMesh.iniciar(this)   // crea la cuenta la primera vez
```

El núcleo en Rust ya va dentro (teléfonos de 64 bits, `arm64-v8a`). [Guía del SDK de Android](docs/sdk.md) · ejemplo [`examples/notas`](examples/notas).

### Web

**1. Un solo comando** en la carpeta de tu proyecto (no hace falta instalar Rust ni compilar nada):

```sh
curl -sL https://github.com/juanmanuel767/mobilemesh/releases/latest/download/mobilemesh-web.tar.gz | tar xz
```

**2. Úsalo:**

```js
import { MobileMesh } from "./mobilemesh/mobilemesh.js";

const mesh = await MobileMesh.iniciar();                    // crea la cuenta la primera vez
await mesh.guardar("tareas", "t1", "titulo", "Comprar pan"); // cifrado y en el dispositivo
console.log(await mesh.datos("tareas"));
```

**3. Dos cabeceras** en tu servidor (el SDK usa memoria compartida):

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Con **Vite** (`vite.config.js`):

```js
export default {
  server: { headers: { "Cross-Origin-Opener-Policy": "same-origin", "Cross-Origin-Embedder-Policy": "require-corp" } },
};
```

Para probar sin nada más: `python3 mobilemesh/servir.py 8080` y abre `http://127.0.0.1:8080`. Mensajes, contactos y colecciones compartidas: [guía del SDK web](docs/sdk-web.md).

## ⚙️ Cómo funciona

```mermaid
flowchart LR
    subgraph Usuarios["📱 Apps de tus usuarios"]
        AN["Android / React Native"]
        WB["Web"]
    end
    AN <-->|"directo (misma red)"| WB
    AN <-->|cifrado E2E| SN["🌐 Supernodos<br/>(móviles de usuarios)"]
    AN <-->|cifrado E2E| RL["🛰️ Relés"]
    WB <-->|cifrado E2E| RL
    RL --- G["🛡️ Guardián<br/>copias cifradas"]
    RL --- N["👤 Guía de nombres"]
    RL --- H["⚡ Ayudantes<br/>(p. ej. pagos)"]
```

- Los datos viven **cifrados en cada dispositivo**. Entre dispositivos viajan cifrados de extremo a extremo: directo en la misma red, o por un **relé** que no puede leerlos.
- El **guardián** guarda copias cifradas que no puede abrir; la copia se actualiza sola tras cada cambio.
- La **guía de nombres** solo sabe `nombre → dirección` (datos públicos); nadie puede quitarle el nombre a su dueño, y **cada app tiene sus propios nombres**.
- Los **supernodos** son móviles de usuarios que ayudan a conectar a otros; un **director** comprueba que se llega a ellos antes de recomendarlos.
- Los **ayudantes** son funciones de servidor opcionales que saben con certeza qué cuenta los llama: [pagos con cualquier pasarela](examples/ayudante-pagos).

## ✅ Estado

**Probado en móviles reales** (TECNO KJ5, Redmi A5 y nubia Z2465N), en la misma wifi y en redes distintas:

- Cuentas con usuario y contraseña, varios dispositivos por cuenta, identidad protegida con Android Keystore.
- Usuarios por nombre y solicitudes de contacto que se aceptan solas.
- Mensajes cifrados de extremo a extremo (Double Ratchet) en tiempo real; fotos, vídeos y documentos cifrados que continúan tras un corte.
- Base de datos local cifrada, sincronización entre dispositivos, modo sin conexión, listas compartidas con permisos.
- Recibir con la app cerrada y **avisos al instante sin Google**, también con la pantalla apagada (se pausan solos con batería por debajo del 15 %).
- Copia automática en el guardián; recuperar la cuenta con contraseña o con código en papel (probado formateando un móvil).
- Pagos con Wompi (modo de pruebas) con regreso automático a la app.
- Relé que no puede leer nada, mensajes rellenados a tamaño fijo, bloqueo de contactos, límites contra el spam, rechazo de impostores.
- Supernodos: un móvil en wifi hace de relé para otros con datos móviles y deja de hacerlo solo al bajar del 15 % de batería.

**Pendiente:** iPhone · usuarios por nombre en la web · supernodos en la web · más guardianes y guías públicos (hoy dependen de un solo móvil) · liberar nombres abandonados · avisos automáticos de Wompi (webhooks) · medir el gasto de batería de un supernodo.

## 🧑‍💻 Para programadores

| | |
|---|---|
| React Native | [docs/react-native.md](docs/react-native.md) |
| Android | [docs/sdk.md](docs/sdk.md) · [`examples/notas`](examples/notas) |
| Web | [docs/sdk-web.md](docs/sdk-web.md) |
| Pagos | [`examples/ayudante-pagos`](examples/ayudante-pagos) |
| IA (Claude, ChatGPT…) | [servidor MCP](docs/mcp.md) |
| Protocolo y seguridad | [protocolo](docs/protocolo.md) · [reglas y amenazas](docs/fase-0.md) · [dependencias y licencias](docs/dependencias.md) |

### Probar el núcleo

```sh
cd core
cargo test --release            # 75 pruebas, entre ellas 8 ataques simulados
cargo build --release
M=target/release/mesh
$M /tmp/ana init && $M /tmp/beto init
$M /tmp/ana contacto beto $($M /tmp/beto id)
$M /tmp/beto contacto ana $($M /tmp/ana id)
$M /tmp/beto escuchar 127.0.0.1:7801 &
$M /tmp/ana enviar beto 127.0.0.1:7801 "hola"
$M /tmp/beto mensajes
```

### Tu propio relé, guardián o guía

```sh
cd servidor && cargo build --release && target/release/mesh-rele 0.0.0.0:7900   # relé
core/target/release/mesh <carpeta> guardian 0.0.0.0:7954 <ruta pública>          # guardián
core/target/release/mesh <carpeta> nombres 0.0.0.0:7955 <ruta pública>           # guía de nombres
```

O en un móvil Android: `sh android/rele-movil/construir.sh` (las tres piezas en una app).

<details>
<summary><b>Decisiones de diseño</b> (por qué está hecho así)</summary>

- [0001 Tecnologías](docs/decisiones/0001-tecnologias.md)
- [0002 Identidad, cuentas y recuperación](docs/decisiones/0002-identidad-y-recuperacion.md)
- [0003 Cifrado de extremo a extremo](docs/decisiones/0003-cifrado-de-extremo-a-extremo.md)
- [0004 Fotos, vídeos y documentos](docs/decisiones/0004-archivos.md)
- [0005 Sin conexión y sincronización](docs/decisiones/0005-sincronizacion.md)
- [0006 Copias en otros dispositivos](docs/decisiones/0006-copias-en-otros-dispositivos.md)
- [0007 Relé y metadatos](docs/decisiones/0007-rele-y-metadatos.md)
- [0008 Permisos](docs/decisiones/0008-permisos.md)
- [0009 Avisos y segundo plano](docs/decisiones/0009-avisos-y-segundo-plano.md)
- [0010 SDK](docs/decisiones/0010-sdk.md)
- [0011 Auditoría](docs/decisiones/0011-auditoria.md)
- [0012 Guardián](docs/decisiones/0012-guardian.md)
- [0013 Directorio](docs/decisiones/0013-directorio.md)
- [0014 Auditoría de las piezas nuevas](docs/decisiones/0014-auditoria-piezas-nuevas.md)
- [0015 Guía de nombres y conexión en carrera](docs/decisiones/0015-guia-de-nombres.md)

</details>

## Licencias y autor

El núcleo y los SDK (`core/`, `android/mobilemesh-sdk`, `react-native/`, `web/`) son [MIT](LICENSE): úsalos en cualquier app, también de pago. Los programas de servidor (`servidor/` y `android/rele-movil`) son [AGPL-3.0](servidor/LICENSE): si los ofreces como servicio modificados, publica tus cambios.

Creado por **Juan Manuel Peralta Chacon** ([@juanmanuel767](https://github.com/juanmanuel767)).
