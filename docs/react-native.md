# Guía de MobileMesh para React Native (Android)

Base de datos, cuentas por nombre de usuario, mensajes, archivos y avisos, sin servidores de datos.

## 1. Instalar (un comando)

```sh
npm install github:juanmanuel767/mobilemesh#v0.1.9
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

// Código en papel (56 letras): enséñalo al crear la cuenta para que lo apunten
const codigo = await MobileMesh.codigoPapel();
// Contraseña olvidada: entrar con el código y elegir una contraseña nueva
await MobileMesh.entrarConCodigo("ana", codigo, "contraseña-nueva-12");
// ¿Se puede recuperar al formatear? Si no (sin red al registrarse), protegerla:
if (!(await MobileMesh.protegida())) await MobileMesh.proteger("contraseña-larga-12");

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

// Cobrar (con un ayudante de pagos, ver examples/ayudante-pagos): abre la pasarela, vuelve sola a
// la app y espera el resultado. La app no tiene que declarar nada en Android.
const { estado } = await MobileMesh.pagar("cafe");   // "APROBADO" | "RECHAZADO" | …

// Base de datos cifrada en el móvil
await MobileMesh.guardar("tareas", "t1", { titulo: "Comprar pan" });
console.log(await MobileMesh.datos("tareas"));       // [{id: "t1", campos: {titulo: "Comprar pan"}}]

// Colección pública: lo que la app publica para que lo vean todos (ranking, catálogo)
await MobileMesh.publicar("ranking", "ana", { nombre: "Ana", puntos: 120 });
const top = await MobileMesh.consultarPublico("ranking", { orden: "puntos", desc: true, limite: 10 });
// [{id: "beto", autor: "<cuenta>", campos: {nombre: "Beto", puntos: 300}}, …]
```

Los avisos de mensajes nuevos funcionan con la app cerrada (servicio propio, sin Google).

La copia cifrada de la cuenta (contactos, mensajes y datos) se actualiza sola en el guardián un
minuto después de cada cambio: al formatear, «entrar» o «entrarConCodigo» la recuperan.

## Probado

- TECNO KJ5 ⇄ Redmi A5 con la app de ejemplo ChatMesh: cuentas por nombre, solicitudes aceptadas
  solas, mensajes en los dos sentidos (misma wifi y redes distintas), fotos y avisos.
- Pago con Wompi (modo de pruebas) con `pagar()`: la pasarela aprueba y la app muestra «Pagado».
- Redmi formateado (datos borrados): recuperados cuenta, contacto y mensajes con contraseña y con
  el código en papel.

## Pendiente

- iPhone.
