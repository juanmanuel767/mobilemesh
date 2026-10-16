# Guía de MobileMesh para la web

El mismo núcleo que en Android, compilado a WebAssembly. Los datos se guardan cifrados en el
navegador (OPFS) y los mensajes viajan cifrados de extremo a extremo por los relés. Sin servidores
de datos, sin Firebase, sin Supabase.

```
Tu página ⇄ trabajador con el núcleo (WebAssembly) ⇄ WebSocket ⇄ relé ⇄ móviles y otras webs
```

## 1. Preparar

**Lo fácil (recomendado):** un solo comando, ya compilado:

```sh
curl -sL https://github.com/juanmanuel767/mobilemesh/releases/latest/download/mobilemesh-web.tar.gz | tar xz
```

**Compilarlo tú:**

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
sh web/construir.sh            # genera web/pkg
```

Copia a tu web: `mobilemesh.js`, `red.js`, `nucleo-worker.js` y la carpeta `pkg/`.

## 2. Cabeceras obligatorias

El núcleo espera a la red como en el móvil; para eso necesita memoria compartida, que el navegador
solo da con estas dos cabeceras en tu página:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

(`web/servir.py` las pone para probar en local.)

## 3. Usar

```js
import { MobileMesh } from "./mobilemesh.js";

const mesh = await MobileMesh.iniciar();
console.log("mi cuenta:", await mesh.cuenta());

// Datos (cifrados y permanentes en este navegador)
await mesh.guardar("tareas", "t1", "titulo", "Comprar pan");
console.log(await mesh.datos("tareas"));   // [{id: "t1", campos: {titulo: "Comprar pan"}}]

// Contactos y mensajes
const miInvitacion = await mesh.invitacion();     // compártela (QR, enlace…)
await mesh.anadirContacto("ana", invitacionDeAna);
await mesh.enviar("ana", "hola");                 // "entregado" | "en buzón" | "pendiente: …"
mesh.recibir(async (n) => console.log(await mesh.mensajes()));

// Colecciones compartidas con permisos
await mesh.darPermiso("tareas", "ana", "escribir"); // "leer" | "escribir" | "nada"
await mesh.sincronizar("ana");
```

## 4. Diferencias con Android

| | Android | Web |
|---|---|---|
| Recibir con la app cerrada | Sí (servicio en segundo plano) | No: solo con la página abierta; lo que llegue mientras tanto espera en el buzón del relé (7 días) |
| Conexión directa entre dispositivos | Sí (misma wifi, UPnP, IPv6) | No: siempre por el relé |
| Clave de la cuenta | En el chip de seguridad (Keystore) | En el almacén privado de la web (OPFS) |
| Páginas `https://` | — | Funcionan: el relé acepta `wss://` (candado) en el mismo puerto 7901 |

## Probado

- Navegador real (Chromium): cuenta y datos guardados tras recargar la página.
- Navegador ⇄ PC y navegador ⇄ TECNO (Android) por el relé del nubia, en los dos sentidos, en
  menos de 1 segundo, con confirmación de entrega.
- Directorio firmado descargado y comprobado dentro del navegador.
- Desde una página `https://` real, `wss://mobilemeshjuan.duckdns.org:7901`: conexión en ~70–260 ms
  y respuesta en 24–37 ms.

## Pendiente

- Envolver la clave de la cuenta con WebCrypto (no exportable).
- Archivos (fotos, vídeos) y recuperación con guardián desde la web.
