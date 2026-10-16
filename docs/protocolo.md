# Protocolo de MobileMesh (versión 2)

Descripción para quien quiera revisar el protocolo o hacer una implementación compatible.
Código de referencia: `core/src` (Rust).

## 1. Identidades

- **Cuenta:** clave de firma **Ed25519**. Su clave pública (32 bytes, hexadecimal) es el identificador
  que conocen los contactos.
- **Dispositivo:** clave **X25519** propia de cada teléfono (la de Noise).
- **Certificado de dispositivo:** firma Ed25519 de la cuenta sobre `"mobilemesh-dispositivo-v1" ||
  clave_pública_del_dispositivo`.
- **Vinculación:** el dispositivo nuevo muestra `mobilemesh-vincular:1:<clave dispositivo>`; el
  principal devuelve `mobilemesh-certificado:1:<cuenta>:<firma>`. Un código corto (48 bits de la clave
  del dispositivo) se compara a simple vista en las dos pantallas.
- **Invitación:** `mobilemesh:2:<cuenta>:<ruta>`. La ruta es `ip:puerto`, `relay=host:puerto/buzón`
  o varias separadas por `|` (se prueban en orden).

## 2. Conexión

1. TCP directo o empalmado por un relé (sección 7).
2. **Noise_XX_25519_ChaChaPoly_BLAKE2s**, prólogo `mobilemesh/2`, paquetes con 2 bytes de longitud.
3. La **prueba** (cuenta 32 bytes || certificado 64 bytes) viaja cifrada en los mensajes 2 y 3 del
   saludo. El iniciador comprueba la del respondedor **antes** de enviar la suya.
4. Plazos: 15 s en total para el saludo y 30 s en total por paquete.
5. Tras el saludo, cada lado manda un paquete `Rev` con la **lista de dispositivos retirados** de su
   cuenta (JSON firmado por la cuenta, con versión). Se rechaza la conexión si el otro dispositivo
   está retirado o la cuenta bloqueada.

Cada paquete posterior es JSON dentro de Noise, precedido de 4 bytes de longitud y **rellenado con
ceros hasta un múltiplo de 512 bytes**.

## 3. Mensajes (cifrado de extremo a extremo)

- Sesión **Olm / Double Ratchet** (vodozemac) por cada pareja de dispositivos. Si no la hay:
  `KeysReq` → `Keys { identity, one_time }` (clave de un solo uso).
- `Msg { id, kind, body }`: `body` es el mensaje Olm en base64. Dentro va `{id, body, file?}`; el
  `id` (32 hex) se comprueba contra el de fuera.
- Respuestas: `Ack { id }` (el destino ya lo guardó), `NoSession { id }` (abrir sesión nueva) o
  `Bad { id, reason }`. Un `reason` que empieza por `LÍMITE` significa «reintenta más tarde».
- Límites: 60 mensajes por minuto por contacto; 20 peticiones de claves por minuto.

## 4. Archivos

- Clave aleatoria por archivo; trozos de 32 KB cifrados con **ChaCha20-Poly1305**, con el número de
  trozo en el nonce y en los datos autenticados.
- La clave, el nombre, el tamaño y el **SHA-256** del archivo van dentro del mensaje Olm.
- `Have { id, chunks }` (cuántos trozos tiene ya el destino) → `Chunk { id, index, data }` → `Ack` o
  `Bad`. Las transferencias continúan donde se cortaron.

## 5. Sincronización

- Operación: `{device, seq, collection, record, field, value?, hlc, base, policy, resolves, author}`.
- **Reloj híbrido** (`hlc`: milisegundos << 16 | contador).
- Reglas: `Lww` (por campo, gana el `(hlc, device)` mayor) y `Strict` (cambios simultáneos quedan en
  conflicto; solo los cierra quien los tuvo delante, listados en `resolves`).
- Mensajes `Sync { id, kind, body }` (cifrados con Olm): `Hello { vector, collections? }` →
  `Data { ops, more, vector }`… → `Done { vector }`.
- Entre dispositivos de la misma cuenta se sincroniza todo. Con un contacto, solo las colecciones
  compartidas, y solo se aceptan operaciones donde puede escribir, a su nombre (`author`), con un
  dispositivo ligado a su cuenta, con `seq < 2^53`, un salto razonable y un reloj no más de un día
  adelantado.

## 6. Copias en otros dispositivos

- La copia (contactos, mensajes, operaciones) se cifra como un archivo. Sus datos se envuelven con
  `SHA-256("mobilemesh-copias-v1" || secreto de cuenta)`, con la cuenta y la versión autenticadas.
- `ReplicaPut { version, meta, size }` + trozos; `ReplicaCheck { nonce }` → `ReplicaProof { version,
  proof = SHA-256(nonce || copia) }`; `ReplicaGet`; `ReplicaDel`.
- Copia de recuperación: `MM3-` + hex(sal 16 || nonce 12 || cifrado), con **Argon2id** (64 MiB,
  3 pasadas) y ChaCha20-Poly1305. Se siguen aceptando las `MM2-` (Argon2id 19 MiB, 2 pasadas).

## 7. Funciones de servidor (ayudantes)

Mensajes normales (sección 3) con un cuerpo que empieza por el carácter `\u0001`:
`\u0001mm-llamada:{funcion, datos, ruta}` y `\u0001mm-respuesta:{llamada, ok, resultado}`, donde
`llamada` es el id del mensaje de la llamada y `ruta` dice cómo devolver la respuesta. No se muestran
ni cuentan como mensajes nuevos. Un ayudante con el ajuste `abierto` acepta conexiones de cuentas
desconocidas y las apunta como contactos. Datos y resultado: como mucho 32 KB.

## 8. Relé

Líneas de texto al empezar cada conexión TCP:

- `REGISTRAR <clave pública Ed25519 del buzón>` → `RETO <32 bytes hex>` → `FIRMA <firma de
  "mobilemesh-buzon-v1" || reto>` → `OK <buzón>`. El buzón son los 16 primeros bytes del SHA-256 de la
  clave pública. Luego llegan `LLAMADA <ficha>` y el dueño manda `PING` cada 25 s.
- `MARCAR <buzón>` → `OK` (conexión empalmada) o `NO`.
- `ACEPTAR <ficha>` → la conexión queda empalmada con quien marcó.
- **Buzón de cartas** (para quien está dormido): `DEPOSITAR <buzón> <carta base64>` → `OK` o `NO`.
  `RECOGER <clave pública>` → reto y firma como al registrar → `CARTA <ficha> <carta>`… `FIN`; el dueño
  contesta `RECIBIDA <ficha>` y el relé la borra. La carta es JSON `{t: "msg"|"ack", de, id, kind,
  body}` con `body` cifrado con Olm; solo se acepta de dispositivos cuya cuenta se comprobó antes en
  un saludo. Límites: 48 KB por carta, 200 por buzón, 64 MB en total, 7 días.
- **Avisos (opcional):** `AVISO <clave pública>` → reto y firma → `<ficha>` (o `-` para quitarla) →
  `OK`. Si el relé se arrancó con `--avisos <programa>`, al llegar una carta a ese buzón ejecuta el
  programa con la ficha como argumento (como mucho uno cada 20 s por buzón). No lleva contenido.
- Límites: 1024 conexiones (cambiable con `--max`), 1000 por IP, 1000 buzones por IP (con datos móviles muchos teléfonos comparten IP) y 4 llamadas en espera por buzón.
