# Rendimiento de MobileMesh (medido)

Todo medido con el núcleo de verdad: cifrado, SQLite en disco, Noise y ratchet. Se puede repetir:

```sh
cd core
cargo test --release --test velocidad -- --nocapture   # velocidad
cargo test --release --test medidas -- --nocapture     # tamaños en red y en disco
```

## En un PC (portátil de 2015: Intel Core i5-5200U, 4 hilos)

Valores de 3 repeticiones (mínimo – máximo).

| Operación | Antes | Ahora |
|---|---|---|
| Escribir 1000 campos, uno a uno (cifrado + disco) | 403 – 479 ms | **184 – 217 ms** (≈ 0,2 ms cada uno) |
| Leer una colección de 1000 registros (descifrando) | 54 – 59 ms | **29 – 34 ms** |
| Consulta con filtro + orden + límite sobre 1000 | 55 – 63 ms | **30 – 37 ms** |
| Sincronizar 1000 cambios con otro dispositivo | 538 – 644 ms | **280 – 375 ms** |
| Entregar un mensaje cifrado de extremo a extremo, con confirmación de guardado | ≈ 89 ms | **≈ 1 ms** (50 mensajes en 42 – 51 ms) |

### Qué se mejoró

- **Mensajes, 89 veces más rápidos.** Cada paquete se enviaba en dos trozos (tamaño y contenido) y TCP
  retenía el segundo ≈ 40 ms esperando la confirmación del primero (algoritmo de Nagle + ACK
  retardado). Ahora va en una sola escritura y las conexiones no esperan (`TCP_NODELAY`).
- **Base de datos, el doble de rápida.** SQLite en modo WAL: menos escrituras al disco, igual de
  seguro ante apagones (`synchronous` sigue en `FULL`), y la app y su servicio pueden leer mientras
  el otro escribe. En el navegador no aplica (su almacén OPFS no lo admite).

## Tamaños

| | |
|---|---|
| Mensaje corto por la red (50 en una conexión) | 1 089 bytes cada uno (relleno a 512 bytes para no delatar la longitud) |
| Conexión + primer mensaje (saludo, listas y ratchet) | 3 570 bytes |
| Sincronización sin cambios | 3 570 bytes |
| Base de datos del receptor | ≈ 401 bytes por mensaje |
| Archivo de 1 MB por la red | 1,35 MB (cifrado por trozos con comprobación) |
| Librería de Android (`.aar`) | 2,3 MB |
| SDK web (descarga) | 1,1 MB |

## En la red real

| Prueba | Resultado |
|---|---|
| Navegador ⇄ PC por el relé (internet) | 0,5 – 0,6 s por mensaje, con confirmación |
| Navegador → móvil Android por el relé | ≈ 1 s, con confirmación |
| Móvil con datos → móvil en casa, directo (sin relé) | conexión en 349 ms |
| Página `https://` → relé con candado (`wss://`) | conexión 66 – 263 ms, respuesta 24 – 37 ms |
| Relé en un móvil (nubia, Android 16) | 4 752 conexiones a la vez con 62 MB de memoria |
| Aviso de mensaje nuevo, pantalla apagada 5 min (móvil cargando) | < 1 s |

## Lo que aún puede mejorar

- **Escribir: ≈ 0,2 ms por campo.** Cada campo se cifra y se registra para sincronizarlo; SQLite a
  secas (sin cifrar ni sincronizar) es más rápido. Agrupar varios campos en una transacción lo
  acercaría.
