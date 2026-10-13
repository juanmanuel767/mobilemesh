# Decisión 0007: conexión entre redes distintas, relé y metadatos (Fase 9)

Fecha: 12-10-2026 · Estado: aceptada

## Los tres mecanismos (como pide el plan)

| Mecanismo | Cómo lo resuelve MobileMesh hoy | ¿Infraestructura? |
|---|---|---|
| **Descubrimiento** | La **invitación** (QR) lleva la ruta: dirección directa y, si hay, la ruta por el relé. No hay directorio central de usuarios | Ninguna |
| **Señalización** | El propio relé: avisa al dueño del buzón de que alguien llama («LLAMADA») | El relé |
| **Retransmisión** | El relé **empalma** dos conexiones salientes y reenvía bytes cifrados de extremo a extremo | El relé, solo si la directa falla |

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Relé | Programa aparte y pequeño, `mesh-rele`, que cualquiera puede ejecutar (casa, servidor propio, comunidad) | Nadie depende de una empresa. Principio 6 de la Fase 0. |
| Atravesar routers y datos móviles | Cada dispositivo mantiene una conexión **saliente** con el relé (los routers dejan salir); el relé une las llamadas a ella | Funciona detrás de cualquier router, sin abrir puertos. |
| Directa primero | La ruta es «directa \| relé»: se prueba la directa 3 s y, si no, el relé | El plan prefiere P2P; el relé solo si hace falta. |
| Qué ve el relé | Bytes cifrados con Noise de extremo a extremo (el saludo y los mensajes atraviesan el relé) | No puede leer ni suplantar. |
| Buzones | Identificador **al azar**, sin relación con la cuenta; el relé guarda solo la huella del secreto, así que nadie más puede registrar tu buzón | El relé no sabe qué cuenta hay detrás de cada buzón. |
| Tamaño de los mensajes | Todos los paquetes se **rellenan a múltiplos de 512 bytes** | Por el tamaño no se adivina qué se escribió (probado: «si» y «no puedo, mañana» ocupan lo mismo). |
| Registros | El relé no guarda nada ni escribe registros; cuenta solo los bytes reenviados (para medir su coste) | Minimizar lo que se puede filtrar. |

## Lo que el relé SÍ puede ver (no prometemos anonimato)

Direcciones IP de quien llama y de quien recibe, horas, duración y volumen de cada conexión, y qué buzón recibe llamadas de qué IP. Un relé malicioso podría además negarse a conectar (pero no leer ni cambiar nada). Un observador de la red ve las líneas de control (`MARCAR <buzón>`), que van sin cifrar.

## Probado

- 36 pruebas automáticas: dos dispositivos sin conexión directa se entregan mensajes por el relé; un buzón no registrado no lleva a ninguna parte; el buzón no revela su secreto; la invitación con relé es válida; mensajes cortos de distinto largo ocupan lo mismo en la red.
- **En los teléfonos reales**, con `mesh-rele` en el PC y la dirección directa de cada contacto puesta a propósito como inalcanzable (como si estuvieran en redes distintas): los dos teléfonos y el PC registraron su buzón; el PC entregó mensajes al nubia y al TECNO por el relé (3–4 s) y el TECNO al PC.
- **Entre redes realmente distintas:** el nubia con **datos móviles** (dirección `100.98.x.x`, compartida por la operadora: nadie puede conectarse a él desde fuera) y el TECNO y el PC en la wifi de casa. El relé del PC se publicó en internet con un túnel temporal gratuito (bore.pub, que solo reenvía bytes cifrados y se cerró al acabar). Resultados: nubia → PC entregado; PC → nubia entregado en 6 s; sincronización nubia ↔ TECNO en los dos sentidos (el perfil cambiado en cada uno apareció en el otro); cuatro sincronizaciones manuales seguidas correctas en 12–14 s.
- Encontrados y corregidos en esa prueba:
  - Un dispositivo propio que cambiaba de dirección quedaba **duplicado**; ahora se actualiza (con prueba automática).
  - **Grave:** al reiniciarse la app con el teléfono **bloqueado**, Keystore no entrega la clave (a propósito) y la app decía «identidad dañada» y ofrecía empezar de nuevo, lo que habría borrado una identidad sana. Ahora muestra «Teléfono bloqueado» y se abre sola al desbloquear; nunca ofrece borrar en ese caso.
  - Una sincronización manual que coincidió con la automática se cortó una vez; se recuperó sola y no se volvió a repetir.

## Pendiente

- Un relé permanente en un servidor propio (el túnel de la prueba era temporal).
- Recibir con la app en segundo plano o con el teléfono bloqueado (Fase 11): hoy la app tiene que estar abierta y el teléfono desbloqueado al arrancarla.
- **Conexión directa a través de routers** (perforación de NAT, como WebRTC o libp2p) para usar el relé aún menos.
- **Buzones que cambian** cada cierto tiempo (avisando a los contactos por el canal cifrado), varios relés y elegir entre ellos.
- **Guardar mensajes en el relé** para quien está desconectado (cifrados de extremo a extremo con el ratchet).
- Cifrar las líneas de control del relé; límites contra abusos.
- Cambiar de relé hoy requiere reiniciar la app.
