# Decisión 0002: protección de la identidad y recuperación (Fase 2)

Fecha: 12-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Clave en el teléfono | La identidad (X25519) se guarda cifrada con una clave AES-256-GCM de **Android Keystore**, que solo funciona con el teléfono desbloqueado | Keystore no permite sacar su clave: copiar el archivo de identidad a otro equipo no sirve. Noise necesita la clave X25519 en memoria, por eso Keystore la *envuelve* en vez de guardarla directamente. |
| Cómo se conecta | El núcleo recibe un «protector» (`KeyGuard`) que aporta cada plataforma | El núcleo sigue siendo el mismo en Android, iPhone y PC; en PC se usa archivo 0600 sin cifrar (solo para pruebas). |
| Cuenta y dispositivo | Cada **cuenta** tiene una clave de firma Ed25519; cada **dispositivo**, su clave X25519 de Noise. La cuenta firma la clave de cada dispositivo (certificado) y el certificado viaja cifrado dentro del saludo Noise | Los contactos confían en la cuenta y aceptan cualquier dispositivo suyo. Permitirá revocar un dispositivo sin tocar los demás. |
| Vincular otro dispositivo | El nuevo muestra un QR con su clave pública y un código de comprobación (48 bits); el principal lo escanea, el usuario compara el código y el principal le devuelve el certificado en otro QR | No viaja ningún secreto ni hace falta red ni servidor: el certificado solo sirve para ese dispositivo. |
| Quién guarda la clave de la cuenta | Solo el dispositivo **principal** | Mínimo privilegio: un dispositivo vinculado robado no puede vincular otros ni sacar la copia. |
| Copia de recuperación | Clave de la **cuenta** cifrada con **Argon2id + ChaCha20-Poly1305** usando una contraseña de al menos 10 caracteres. Formato `MM2-` + 152 caracteres; también como QR. Al restaurar se crea un dispositivo principal nuevo | Quien fotografíe la copia no puede usarla sin la contraseña; Argon2id hace muy caro probar contraseñas. |
| Añadir contactos | Invitación `mobilemesh:2:<cuenta>:<ip:puerto>` en QR o como texto | Solo datos públicos. Se acabó escribir 64 caracteres a mano. |
| Cambio de clave de un contacto | Se rechaza: hay que añadirlo con otro nombre tras comprobarlo con esa persona | Evita que alguien suplante a un contacto existente en silencio. |
| Base de datos | Migraciones numeradas (`PRAGMA user_version`) | Los teléfonos con datos de la Fase 4 se actualizaron sin perder nada. |

## Probado en teléfonos reales (TECNO KJ5 y nubia Z2465N)

- Actualizar desde la Fase 4: misma identidad; el archivo pasa de 64 bytes sin cifrar a 92 cifrados con Keystore.
- Invitaciones: cada teléfono añadió al otro y se mandaron mensajes eligiendo el contacto de la lista, sin escribir la IP.
- Copia: rechaza contraseñas cortas. Borrado total de la app en el nubia (también su clave de Keystore) → recuperada la **misma** identidad con la copia; con una contraseña errónea se rechaza. El TECNO le volvió a escribir sin cambiar nada por su lado.
- Vinculación: el nubia se unió a la cuenta del TECNO (mismo código de comprobación en las dos pantallas). El PC, con un único contacto para esa cuenta, recibió mensajes de los dos teléfonos y respondió a cada uno.
- Actualización al protocolo 2: cada teléfono conservó su clave de dispositivo y pasó a ser principal de una cuenta nueva; hubo que volver a invitar a los contactos (las copias `MM1-` ya no sirven).

## Pendiente

- **Revocar** un dispositivo vinculado (Fase 10): hará falta una lista de dispositivos firmada por la cuenta y que llegue a los contactos.
- **Entregar a todos los dispositivos** de una cuenta: hoy el mensaje llega al dispositivo cuya dirección se conoce (Fase 7, sincronización).
- **Recuperación asistida** (repartir la copia entre contactos de confianza, p. ej. Shamir).
- Copia en palabras para escribir en papel (tipo BIP39).
- Probar el **escaneo de QR con la cámara** entre los dos teléfonos (necesita apuntar físicamente).
- Cifrar también la base de datos local (hoy solo la protege la carpeta privada de la app).
- Al restaurar se recupera la identidad, no los contactos ni los mensajes (llegará con las réplicas de la Fase 8).
