# Integración de MobileMesh en Cifra

Cifra es una hoja de cálculo web con IA (`/home/user/Descargas/excel`). Usa MobileMesh
como almacén de sus documentos: así las cuentas de una pyme no salen del dispositivo.

Este documento registra **qué se cambió, por qué y qué falta**, para que la próxima
persona (o la próxima sesión) no tenga que volver a averiguarlo.

Fecha: 16 de noviembre de 2026 · SDK web 0.2.1

---

## 1. Qué se integró

| Pieza | Dónde, en Cifra |
|---|---|
| SDK web completo (`mobilemesh.js`, `red.js`, `nucleo-worker.js`, `pkg/`) | `web/public/mobilemesh/` |
| Capa de almacenamiento con dos motores | `web/src/almacen.ts` |
| Cabeceras de aislamiento | `web/vite.config.ts` |
| Pantalla «Mis hojas» | `web/src/ui/DocumentsDialog.tsx` |
| Guardado automático | `web/src/App.tsx` |

### Modelo de datos

Una colección, `cifra_documentos`. Cada documento es un registro con cuatro campos:

| Campo | Contenido |
|---|---|
| `nombre` | Nombre del libro, para listarlo sin abrirlo |
| `actualizado` | Fecha ISO del último guardado |
| `hojas` | Cuántas hojas tiene |
| `documento` | El libro entero en JSON |

El borrado se hace marcando `borrado = "si"` y vaciando `documento`, porque la API
actual guarda campos pero no borra registros.

### Interfaz de almacenamiento

Cifra no habla con MobileMesh directamente. Habla con cuatro métodos:

```ts
interface Almacen {
  guardar(doc: Doc): Promise<void>;
  abrir(id: string): Promise<Doc | null>;
  listar(): Promise<Guardado[]>;
  borrar(id: string): Promise<void>;
}
```

Hay dos implementaciones: `AlmacenMobileMesh` y `AlmacenNavegador` (respaldo con
`localStorage`). Si MobileMesh no arranca, Cifra sigue funcionando y **dice por qué**
en vez de fallar en silencio.

---

## 2. Cambios que hubo que hacer, y por qué

### 2.1 `credentialless` en vez de `require-corp`

El manual (`docs/sdk-web.md`) pide:

```
Cross-Origin-Embedder-Policy: require-corp
```

Cifra usa `credentialless`. Motivo: con `require-corp`, los tipos de letra de Google
Fonts dejan de cargar, porque no envían `Cross-Origin-Resource-Policy`. Con
`credentialless` el navegador sigue dando `crossOriginIsolated: true` (verificado) y
las fuentes cargan sin alojarlas.

**Sugerencia para el manual:** mencionar `credentialless` como alternativa válida
cuando la página carga recursos de otros orígenes.

### 2.2 El SDK va en `public/`, no en `src/`

Vite no puede empaquetar el SDK: el trabajador resuelve `./pkg/mobilemesh_core.js`
y el `.wasm` con `import.meta.url`, y al reempaquetarlo esas rutas se rompen.

Servirlo desde `public/` lo deja tal cual. Pero entonces **no se puede importar con
`import()` normal**: Vite responde

> This file is in /public and will be copied as-is during build without going through
> the plugin transforms, and therefore should not be imported from source code.

La solución que funciona es armar la importación en tiempo de ejecución para que el
empaquetador no la analice:

```ts
const ruta = `${window.location.origin}/mobilemesh/mobilemesh.js`;
const importar = new Function('u', 'return import(u)') as (u: string) => Promise<unknown>;
const modulo = await importar(ruta);
```

**Sugerencia para el manual:** añadir este caso a la sección de Vite. El
`optimizeDeps.exclude` que documenta solo sirve si el SDK está instalado por npm;
mientras el paquete no esté publicado, la vía real es `public/` + importación en
tiempo de ejecución.

### 2.3 Límite de tiempo al arrancar

`MobileMesh.start()` no tiene tiempo máximo. Si el núcleo no arranca, la promesa no
se resuelve **ni se rechaza**: la aplicación se queda esperando para siempre y sin
guardar, sin ningún aviso.

Cifra le pone 8 segundos al arranque y 4 a `account()`, y si no, cae al respaldo.

**Sugerencia para el SDK:** que `start()` acepte un tiempo máximo, o que el trabajador
avise si `init()` tarda más de unos segundos. Un fallo ruidoso es mejor que un cuelgue.

---

## 3. Estado: el núcleo web FUNCIONA

Comprobado en Chrome normal el 16 de noviembre de 2026, con las siete etapas en verde:

```
1. wasm descargado: 3.378.195 bytes (78 ms)
2. wasm compilado sin ejecutarlo (85 ms)
3. importaciones: mobilemesh_core_bg.js ×82, conexion.js ×1
4. init() completo (112 ms)
5. preparar(): OPFS listo (64 ms)
6. cuenta(): 24be5dad96cf46190becac771ca566d3d955c6bdefcec9b75a36ee52a6275160
7. guardar + leer: [{"campos":{"texto":"hola"},"id":"r1"}]
TODO BIEN: el núcleo web funciona
```

El arranque completo tarda **menos de 350 ms**. No necesita relé para guardar en local.

Lo comprobado:

| Requisito | Resultado |
|---|---|
| `crossOriginIsolated` | ✅ true |
| `SharedArrayBuffer` | ✅ disponible |
| `WebAssembly` | ✅ disponible |
| OPFS (`navigator.storage.getDirectory`) | ✅ accesible |
| El trabajador arranca | ✅ sí |
| `pkg/mobilemesh_core.js` se importa | ✅ sí |
| `init()` termina | ✅ 112 ms |
| `preparar()` (OPFS) | ✅ 64 ms |
| Guardar y leer un dato | ✅ sí |

**Todo lo que parecía un fallo del núcleo era un artefacto de medir con Chrome sin
ventana.** Ver la advertencia de abajo antes de depurar nada por ahí.

### Advertencia sobre cómo medirlo

Medir esto con Chrome sin ventana da resultados falsos, y conviene saberlo antes de
perder una tarde:

- Con `--virtual-time-budget`, los temporizadores avanzan pero **la red dentro de un
  trabajador se cuelga**: ni el `fetch` del `.wasm` termina. Cualquier conclusión
  sacada así es un artefacto.
- Sin `--virtual-time-budget`, la red funciona pero **la captura se toma antes** de
  que el diagnóstico termine, así que parece colgado sin estarlo.

Por eso el diagnóstico lleva un reloj en pantalla: sin ver los segundos corriendo, no
se puede distinguir «colgado» de «aún no ha pasado el tiempo».

### Herramienta de diagnóstico

Quedó en el repositorio:

```sh
cd web && python3 servir.py 8099
# abrir en un navegador NORMAL (no headless): http://127.0.0.1:8099/paso-a-paso.html
```

`web/paso-a-paso.html` + `web/paso-a-paso.js` separan el arranque en siete etapas y
miden cada una: descargar el wasm, compilarlo sin ejecutarlo, ver qué importaciones
pide, `init()`, `preparar()`, `cuenta()` y guardar/leer. La etapa en la que se detiene
la lista es la que falla.

---

## 4. Qué falta

1. **Publicar el paquete en npm.** Hoy hay que copiar archivos a mano; con
   `npm install mobilemesh` la integración sería una línea.
2. **Tiempo máximo en `start()`** (ver 2.3).
3. **Borrado real de registros** en la API, para no tener que marcar `borrado = "si"`.
4. **Sincronización entre dispositivos** en Cifra: cuando el arranque funcione, usar
   `shareWith` y `syncWith` para que la misma hoja se vea en el móvil y en el
   portátil, y para compartirla con el contador.

---

## 5. Lo que Cifra gana cuando esto funcione

El argumento comercial de Cifra pasa de «tus datos están seguros en nuestro servidor»
a **«tus datos no salen de tu computador»**. Para un contador que maneja la nómina de
sus clientes, eso se puede cobrar: es un plan distinto, no una viñeta de marketing.

Y MobileMesh gana una aplicación real y exigente como caso de muestra.
