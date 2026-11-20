# MobileMesh

Base de datos y mensajería entre dispositivos **sin servidores**. Los datos viven cifrados en el
dispositivo de cada usuario y viajan cifrados de extremo a extremo entre ellos. Alternativa a
Firebase y Supabase cuando no quieres una nube en medio.

Funciona en cualquier página o aplicación web: React, Vue, Angular, Ionic o JavaScript a secas.

## Instalar

```sh
npm install mobilemesh
```

## Usar

```js
import { MobileMesh } from "mobilemesh";

// La marca de tu app: solo las copias con la misma marca se comunican entre sí.
// Si no la indicas, se usa el dominio de la página.
const mesh = await MobileMesh.start({ appId: "mi-app" });

const tareas = mesh.collection("tareas");
await tareas.save("t1", { titulo: "Comprar pan", hecha: "no" });

const pendientes = await tareas.where("hecha", "==", "no").sortBy("titulo").limit(10).get();
```

Compartir con otra persona:

```js
const invitacion = await mesh.invitation();   // pásasela (QR, enlace, lo que quieras)
await mesh.addContact("ana", invitacionDeAna);
await mesh.shareWith("tareas", "ana", "escribir");
await mesh.sendMessage("ana", "hola");
```

## Un requisito importante

La página debe servirse con estas dos cabeceras, porque el núcleo usa memoria compartida:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

En la [guía del SDK web](https://github.com/juanmanuel767/mobilemesh/blob/master/docs/sdk-web.md)
están las recetas para Vite, Next.js, Angular, Ionic y nginx.

## Tipos

El paquete incluye tipos TypeScript, así que funciona con autocompletado sin instalar nada más.

## Más

- [Contrato de la API](https://github.com/juanmanuel767/mobilemesh/blob/master/docs/api.md)
- [Guía del SDK web](https://github.com/juanmanuel767/mobilemesh/blob/master/docs/sdk-web.md)
- [Repositorio](https://github.com/juanmanuel767/mobilemesh)

Licencia MIT.
