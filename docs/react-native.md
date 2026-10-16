# Guía de MobileMesh para React Native (Android)

Base de datos, cuentas por nombre de usuario, mensajes, archivos y avisos, sin servidores de datos.

## 1. Instalar (un comando)

```sh
npm install github:juanmanuel767/mobilemesh
```

En `android/build.gradle` de tu app: `minSdkVersion = 28`. Recompila la app (`npx react-native run-android`).
El puente baja solo el SDK de Android desde JitPack.

## 2. Usar

```js
import MobileMesh from "react-native-mobilemesh";

// Primera vez: crear la cuenta con usuario y contraseña (la contraseña recupera la cuenta en otro móvil)
await MobileMesh.iniciar();
await MobileMesh.registrar("ana", "contraseña-larga-12");
// En un móvil nuevo, en lugar de lo anterior:
await MobileMesh.entrar("ana", "contraseña-larga-12");

// Hablar con alguien por su nombre (su app te añade sola)
await MobileMesh.anadirPorNombre("juan");
await MobileMesh.enviar("juan", "hola");            // «entregado» | «en buzón»
MobileMesh.alRecibir(async () => console.log(await MobileMesh.mensajes()));
setInterval(() => MobileMesh.actualizar(), 15000);   // solicitudes nuevas y direcciones al día

// Archivos, fotos y documentos
const f = await MobileMesh.elegirArchivo();           // selector de Android
if (f) await MobileMesh.enviarArchivo("juan", f.uri, f.nombre);
const ruta = await MobileMesh.verArchivo(idMensaje);  // file:// para <Image>
await MobileMesh.guardarEnDescargas(idMensaje);

// Base de datos cifrada en el móvil
await MobileMesh.guardar("tareas", "t1", { titulo: "Comprar pan" });
console.log(await MobileMesh.datos("tareas"));       // [{id: "t1", campos: {titulo: "Comprar pan"}}]
```

Los avisos de mensajes nuevos funcionan con la app cerrada (servicio propio, sin Google).

## Probado

- TECNO KJ5 ⇄ Redmi A5 con la app de ejemplo ChatMesh: cuentas por nombre, solicitudes aceptadas
  solas, mensajes en los dos sentidos (misma wifi y redes distintas), fotos y avisos.

## Pendiente

- La guía de nombres universal (en el nubia): mientras, cada app la elige con `usarGuiaDeNombres`.
- iPhone.
