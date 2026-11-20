# Guía de MobileMesh para la web

> Contrato común de la API: [`docs/api.md`](api.md). Todos los SDK ofrecen las mismas operaciones con los mismos nombres.

El mismo núcleo que en Android, compilado a WebAssembly. Los datos se guardan cifrados en el
navegador (OPFS) y los mensajes viajan cifrados de extremo a extremo por los relés. Sin servidores
de datos, sin Firebase, sin Supabase.

```
Tu página ⇄ trabajador con el núcleo (WebAssembly) ⇄ WebSocket ⇄ relé ⇄ móviles y otras webs
```

## 1. Instalar

**Un solo comando**, ya compilado:

```sh
curl -sL https://github.com/juanmanuel767/mobilemesh/releases/latest/download/mobilemesh-web.tar.gz | tar xz
```

**Con npm:** el paquete está preparado en `web/package.json` pero todavía no está publicado. Cuando lo esté, bastará con `npm install mobilemesh` e importar desde `"mobilemesh"`.

Copia a tu web `mobilemesh.js`, `red.js`, `nucleo-worker.js` y la carpeta `pkg/` (deben quedar juntos).

**Compilarlo tú:**

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
sh web/construir.sh            # genera web/pkg y deja web/ listo para npm
```

## 2. Cabeceras obligatorias

El núcleo espera a la red como en el móvil; para eso necesita memoria compartida, que el navegador
solo da con estas dos cabeceras en tu página:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Sin ellas `MobileMesh.start()` lanza un error (`crossOriginIsolated` es falso). Con
`require-corp`, todo recurso de otro origen (imágenes, scripts, fuentes) debe enviar
`Cross-Origin-Resource-Policy: cross-origin` o cargarse con CORS.

**Vite** (`vite.config.js`; también para `vite preview`):

```js
import { defineConfig } from "vite";

const cabeceras = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  server: { headers: cabeceras },
  preview: { headers: cabeceras },
  optimizeDeps: { exclude: ["mobilemesh"] },   // que Vite no reempaquete el worker ni el .wasm
  worker: { format: "es" },
});
```

**Next.js** (`next.config.js`):

```js
module.exports = {
  async headers() {
    return [{
      source: "/(.*)",
      headers: [
        { key: "Cross-Origin-Opener-Policy", value: "same-origin" },
        { key: "Cross-Origin-Embedder-Policy", value: "require-corp" },
      ],
    }];
  },
};
```

Usa MobileMesh solo en el navegador (dentro de `useEffect` o con `import()` dinámico en un
componente de cliente), nunca en el servidor.

**Servidor estático** (nginx):

```nginx
add_header Cross-Origin-Opener-Policy "same-origin" always;
add_header Cross-Origin-Embedder-Policy "require-corp" always;
```

En Apache: `Header always set Cross-Origin-Opener-Policy "same-origin"` y
`Header always set Cross-Origin-Embedder-Policy "require-corp"`. Para probar en local,
`web/servir.py` las pone.

## 3. Usar

```js
import { MobileMesh } from "mobilemesh";   // sin npm: "./mobilemesh.js"

const mesh = await MobileMesh.start({ appId: "mi-web" });   // marca de tu web
console.log("mi cuenta:", await mesh.account());

// Datos (cifrados y permanentes en este navegador)
await mesh.save("tareas", "t1", "titulo", "Comprar pan");
console.log(await mesh.all("tareas"));   // [{id: "t1", campos: {titulo: "Comprar pan"}}]

// Contactos y mensajes
const miInvitacion = await mesh.invitation();     // compártela (QR, enlace…)
await mesh.addContact("ana", invitacionDeAna);
await mesh.sendMessage("ana", "hola");                 // "entregado" | "en buzón" | "pendiente: …"
mesh.onReceive(async (n) => console.log(await mesh.messages()));

// Colecciones compartidas con permisos
await mesh.shareWith("tareas", "ana", "write"); // "read" | "write" | "none"
await mesh.syncWith("ana");
```

La marca (`appId`) es el «lote» de tu aplicación: solo las páginas con la misma marca se comunican
entre sí. Si no la indicas, se usa el dominio de la página (`location.hostname`), que ya es igual
para todos tus usuarios. No la cambies una vez publicada: la base del navegador se guarda por marca.

Si vienes de v0.2.1, el protocolo cambió (buzón del relé, saludo e invitaciones) y las versiones
antiguas no se comunican con las nuevas. Los datos guardados se conservan.

La API está en inglés. Los nombres en español (`iniciar`, `guardar`, `datos`, `enviar`…) siguen funcionando, pero avisan una vez por consola de que están obsoletos. Los permisos también aceptan `"leer"`, `"escribir"` y `"nada"`.

### Colecciones y consultas

`mesh.collection(nombre)` agrupa el guardado y la consulta de documentos. `save(id, campos)` guarda
todos los campos de un objeto (los valores se convierten a texto) y `where` empieza una consulta
encadenable con `sortBy`, `limit` y `get`:

```js
const jugadores = mesh.collection("jugadores");
await jugadores.save("ana", { nombre: "Ana", puntos: "120" });
const todos = await jugadores.all();   // [{ id: "ana", campos: { nombre: "Ana", puntos: "120" } }]
const mejores = await jugadores.where("puntos", ">", "100").sortBy("puntos", true).limit(10).get();
```

Operadores: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza` y `existe`. El formato de cada
documento es `{ id, campos }`, donde `campos` es un objeto de cadenas.

## 4. Integrar en tu framework

En todos los casos hacen falta las cabeceras COOP/COEP de la sección 2. Con npm, el paquete todavía
no está publicado: mientras tanto instala desde el `.tar.gz` de la release o copia `mobilemesh.js`,
`red.js`, `nucleo-worker.js` y `pkg/` al proyecto. Las recetas de Vite y Next.js de la sección 2 se
conservan; estas las reutilizan.

### React (Vite)

```sh
npm create vite@latest mi-app -- --template react
cd mi-app && npm install
```

`vite.config.js`, reutilizando el de la sección 2:

```js
import { defineConfig } from "vite";

const cabeceras = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  server: { headers: cabeceras },
  preview: { headers: cabeceras },
  optimizeDeps: { exclude: ["mobilemesh"] },
  worker: { format: "es" },
});
```

Uso mínimo en un componente (solo en el navegador):

```jsx
import { useEffect } from "react";
import { MobileMesh } from "mobilemesh";

export default function App() {
  useEffect(() => {
    (async () => {
      const mesh = await MobileMesh.start({ appId: "mi-web" });
      await mesh.collection("tareas").save("t1", { titulo: "Comprar pan" });
    })();
  }, []);
  return null;
}
```

### Vue (Vite)

```sh
npm create vite@latest mi-app -- --template vue
cd mi-app && npm install
```

El `vite.config.js` es exactamente el de React (mismas cabeceras, `optimizeDeps.exclude` y
`worker.format`). Uso mínimo:

```vue
<script setup>
import { onMounted } from "vue";
import { MobileMesh } from "mobilemesh";

onMounted(async () => {
  const mesh = await MobileMesh.start({ appId: "mi-web" });
  await mesh.collection("tareas").save("t1", { titulo: "Comprar pan" });
});
</script>
```

### Angular (CLI)

```sh
ng new mi-app --standalone
cd mi-app && npm install mobilemesh
```

En `angular.json`, dentro del proyecto y de `architect.serve.options`, añade las cabeceras del
servidor de desarrollo:

```json
"serve": {
  "options": {
    "headers": {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp"
    }
  }
}
```

Si tu versión de Angular CLI no acepta `headers` en `serve`, sirve `dist/` con un servidor propio
(nginx/Apache o `web/servir.py`) que las añada; un `proxy.conf.json` no sirve para esto porque
gestiona peticiones salientes, no cabeceras de respuesta. Uso mínimo:

```ts
import { Component, OnInit } from "@angular/core";
import { MobileMesh } from "mobilemesh";

@Component({ selector: "app-root", standalone: true, template: "" })
export class AppComponent implements OnInit {
  async ngOnInit() {
    const mesh = await MobileMesh.start({ appId: "mi-web" });
    await mesh.collection("tareas").save("t1", { titulo: "Comprar pan" });
  }
}
```

Si el empaquetador de Angular no copia `nucleo-worker.js` ni `pkg/`, copia esos archivos a
`src/assets/mobilemesh/` y cárgalos desde ahí; no los dejes fuera del build.

### Ionic (Capacitor)

```sh
ionic start mi-app blank --type=react
cd mi-app && npm install mobilemesh
```

En el servidor de desarrollo (`ionic serve`, que usa Vite en React/Vue) añade las mismas cabeceras
en `vite.config.ts`:

```ts
server: {
  headers: {
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "require-corp",
  },
},
```

Uso mínimo, igual que en React/Vue: `MobileMesh.start({ appId: "mi-web" })` dentro de un
`useEffect`/`onMounted` y `collection("tareas").save(...)`.

**Aviso para Android con Capacitor:** las cabeceras del servidor de desarrollo no llegan al WebView
de la app. En Android dentro de Capacitor hay que añadir COOP/COEP a las respuestas del WebView en
el lado nativo; si no, `crossOriginIsolated` es falso y `MobileMesh.start()` lanza el error de
cabeceras. La forma exacta de interceptar esas respuestas depende de la versión de Capacitor y de su
esquema local, así que no la damos aquí como receta fija.

## 5. Diferencias con Android

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
