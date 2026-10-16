<p align="center"><img src="docs/img/logo.png" alt="MobileMesh: base de datos local en tus dispositivos" width="320"></p>

# MobileMesh

<p align="center"><b>🔒 La base de datos que NO vive en la nube.</b><br>Tus datos se quedan en tus dispositivos. Alternativa gratis a Firebase y Supabase.</p>

¿Cuántas veces has leído *«se filtraron los datos de millones de usuarios»*? Pasa porque todo se guarda en un mismo servidor en la nube: un solo fallo y se lo llevan todo. MobileMesh funciona al revés:

| | |
|---|---|
| ☁️❌ **Nada vive en la nube** | Los datos de tu app se guardan cifrados en los dispositivos de tus usuarios: móvil, tablet o navegador. |
| 🛡️ **No hay un servidor central que hackear** | No existe una gran base de datos con la información de todos. Para robar los datos de un usuario habría que robarle el móvil… y aun así están cifrados. |
| 🔐 **Cifrado de extremo a extremo, como Signal** | Ni el creador, ni los relés que conectan los móviles, ni nadie en medio puede leer nada. |
| 📶 **Funciona sin internet** | La app sigue funcionando sin conexión y se sincroniza sola al volver. |
| 💸 **Gratis de verdad** | Sin facturas por usuarios, lecturas o almacenamiento: los datos no ocupan servidores. |
| 📱 **Si pierdes el móvil, no pierdes nada** | Recuperas tu cuenta y tus datos con usuario y contraseña. |
| 🧑‍💻 **Para programadores** | SDK para Android y web, y servidor MCP para que una IA (Claude, ChatGPT) cree apps con él. |

Es un proyecto joven. Si te gusta, pruébalo, déjale una ⭐ y cuéntanos qué mejorarías.

📊 **Sin promesas vacías:** [comparación honesta con Firebase, Supabase y SQLite](docs/comparacion.md) (también en qué *no* gana) · [rendimiento medido](docs/rendimiento.md) (repetible con un comando).

## 🚀 Empieza en 1 minuto (Android)

En `settings.gradle.kts` y en el `build.gradle.kts` de tu app:

```kotlin
dependencyResolutionManagement { repositories { google(); mavenCentral(); maven("https://jitpack.io") } }

dependencies { implementation("com.github.juanmanuel767:mobilemesh:v0.1.3") }
```

```kotlin
val mesh = MobileMesh.iniciar(this)   // crea la cuenta la primera vez
```

Nada más: el núcleo en Rust ya va dentro (teléfonos de 64 bits, `arm64-v8a`). [Guía del SDK de Android](docs/sdk.md).

## 🚀 Empieza en 1 minuto (web)

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

Para probar sin nada más: `python3 mobilemesh/servir.py 8080` y abre `http://127.0.0.1:8080`.

Ya está: tu app guarda datos cifrados en el dispositivo, sin nube y sin configurar servidores (usa los relés públicos). Mensajes, contactos y colecciones compartidas: [guía del SDK web](docs/sdk-web.md).

---

Backend de código abierto donde **los propios dispositivos guardan sus datos** y se comunican directamente y cifrados, sin base de datos central obligatoria. Pensado para usarse **en lugar de Firebase o Supabase**.

Creado por **Juan Manuel Peralta Chacon** ([@juanmanuel767](https://github.com/juanmanuel767)).

**Licencias:** el núcleo y los SDK (`core/`, `android/mobilemesh-sdk`, `web/`) son [MIT](LICENSE): úsalos en cualquier app, también de pago. Los programas de servidor (`servidor/`: relé, puente web y candado; `android/rele-movil`) son [AGPL-3.0](servidor/LICENSE): si los ofreces como servicio modificados, publica tus cambios.

**¿Eres programador?**
- Android: [guía del SDK](docs/sdk.md) y el ejemplo [`examples/notas`](examples/notas).
- Web: [guía del SDK web](docs/sdk-web.md) (WebAssembly, funciona en páginas `https://`).
- IA (Claude, ChatGPT…): [servidor MCP](docs/mcp.md).
- ¿Quieres ayudar? [Dona un relé](docs/donar-un-rele.md) con un móvil viejo.

```
📱 Apps Android ─┐                         ┌─ Guardián: recuperar cuenta y datos
                 ├─► Relés (directorio firmado) ─┤
🌐 Apps web ─────┘   cifrado de extremo a extremo └─ Nadie puede leer los datos
```

- **Relés universales, guardián, directorio, candado y web:** [0012 guardián](docs/decisiones/0012-guardian.md) · [0013 directorio](docs/decisiones/0013-directorio.md) · [0014 auditoría de las piezas nuevas](docs/decisiones/0014-auditoria-piezas-nuevas.md)
- Reglas y amenazas: [docs/fase-0.md](docs/fase-0.md)
- Tecnologías elegidas: [docs/decisiones/0001-tecnologias.md](docs/decisiones/0001-tecnologias.md)
- **Auditoría y lanzamiento (Fase 13):** [docs/decisiones/0011-auditoria.md](docs/decisiones/0011-auditoria.md) · [Protocolo](docs/protocolo.md) · [Dependencias y licencias](docs/dependencias.md)
- SDK para desarrolladores (Fase 12): [docs/decisiones/0010-sdk.md](docs/decisiones/0010-sdk.md)
- Recibir con la app cerrada y avisos (Fase 11, en curso): [docs/decisiones/0009-avisos-y-segundo-plano.md](docs/decisiones/0009-avisos-y-segundo-plano.md)
- Permisos (Fase 10): [docs/decisiones/0008-permisos.md](docs/decisiones/0008-permisos.md)
- Relé, redes distintas y metadatos (Fase 9): [docs/decisiones/0007-rele-y-metadatos.md](docs/decisiones/0007-rele-y-metadatos.md)
- Copias cifradas en otros dispositivos (Fase 8): [docs/decisiones/0006-copias-en-otros-dispositivos.md](docs/decisiones/0006-copias-en-otros-dispositivos.md)
- Sin conexión y sincronización (Fase 7): [docs/decisiones/0005-sincronizacion.md](docs/decisiones/0005-sincronizacion.md)
- Fotos, vídeos y documentos (Fase 6): [docs/decisiones/0004-archivos.md](docs/decisiones/0004-archivos.md)
- Cifrado de extremo a extremo (Fase 5): [docs/decisiones/0003-cifrado-de-extremo-a-extremo.md](docs/decisiones/0003-cifrado-de-extremo-a-extremo.md)
- Identidad, cuentas, vinculación y recuperación (Fase 2): [docs/decisiones/0002-identidad-y-recuperacion.md](docs/decisiones/0002-identidad-y-recuperacion.md)

## Estado

| Hecho | Pendiente |
|---|---|
| **Probado entre dos teléfonos reales (TECNO KJ5 Android 13 y nubia Z2465N Android 16) por wifi, sin servidor.** Cuentas con varios dispositivos vinculados por QR, identidad protegida con Android Keystore, copia de recuperación cifrada con contraseña, contactos por invitación QR, base de datos local con migraciones, cifrado de extremo a extremo por mensaje (Double Ratchet) dentro de una conexión directa cifrada y autenticada, base de datos cifrada, envío de fotos, vídeos y documentos cifrados que continúa tras un corte, datos sincronizados entre los dispositivos de la cuenta con modo sin conexión y conflictos a la vista, reintentos automáticos, copias de seguridad cifradas guardadas por contactos o dispositivos propios que ellos no pueden leer, con comprobación sin descarga, recuperación tras perder el teléfono y borrado, relé autoalojable (`mesh-rele`) para redes distintas que no puede leer nada, mensajes rellenados a tamaño fijo, retirar dispositivos perdidos con una lista firmada, bloquear contactos, límites contra el spam, listas compartidas con permiso de lectura o escritura, servicio en segundo plano con avisos sin contenido, librería para desarrolladores (`mobilemesh-sdk`) con app de ejemplo, confirmación de guardado, cola de pendientes sin duplicados, rechazo de desconocidos y de impostores | Revocar dispositivos, entregar a todos los dispositivos de una cuenta, recuperación asistida, recibir con la app en segundo plano (Fase 11), relé permanente en un servidor propio, perforación de NAT, copiar los mensajes entre los dispositivos de la cuenta, réplicas, avisos push, SDK |

## Probar

```sh
cd core
cargo test                      # 68 pruebas, entre ellas 9 ataques simulados (cargo test --release --test medidas -- --nocapture para las mediciones)
cargo build
M=target/debug/mesh
$M /tmp/ana init && $M /tmp/beto init
$M /tmp/ana contacto beto $($M /tmp/beto id)
$M /tmp/beto contacto ana $($M /tmp/ana id)
$M /tmp/beto escuchar 127.0.0.1:7801 &          # otra terminal o en segundo plano
$M /tmp/ana enviar beto 127.0.0.1:7801 "hola"
$M /tmp/beto mensajes
```

## Relé

```sh
core/target/debug/mesh-rele 0.0.0.0:7900     # en cualquier equipo con dirección alcanzable
```

En la app: «Relé para otras redes» → `ip:puerto` → «Usar este relé» y reiniciarla. La invitación incluye entonces la ruta por el relé.

## App de Android

```sh
android/build.sh            # compila el núcleo, la librería, la app de pruebas y la app de ejemplo
android/build.sh instalar   # además lo instala y lo abre en el teléfono conectado por cable
```

La app recibe en el puerto 7801. Para hablar con el PC: en el PC `mesh <carpeta> escuchar 0.0.0.0:7802`, y en la app pegar la invitación que da `mesh <carpeta> invitacion <ip-del-pc>:7802`.
