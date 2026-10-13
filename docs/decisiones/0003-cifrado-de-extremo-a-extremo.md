# Decisión 0003: cifrado de extremo a extremo (Fase 5)

Fecha: 12-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Cifrado de cada mensaje | **Double Ratchet de Olm** con **vodozemac 0.11.1** (Matrix/Element, auditada; Apache-2.0) | No inventamos criptografía. Cada mensaje usa una clave nueva: robar una no permite leer los anteriores, y tras unas respuestas tampoco los siguientes. |
| Dos capas | Olm va **dentro** del canal Noise | Hoy los dos cifran; mañana un relé u otro teléfono podrá guardar el mensaje Olm sin leerlo (Fases 8 y 9). |
| Abrir una sesión | Por el canal ya autenticado, el emisor pide una **clave de un solo uso** al receptor (`KeysReq`/`Keys`) | Las claves llegan autenticadas por el certificado de la cuenta. Publicarlas para recibir sin conexión llegará con los relés. |
| Una sesión por dispositivo | Cada dispositivo de una cuenta tiene sus propias claves y su propio ratchet | Un dispositivo robado no puede leer lo enviado a los demás. |
| Cambio de claves | La clave Olm de cada dispositivo se **fija la primera vez**; si cambia, se rechaza | Detecta la manipulación en vez de aceptarla en silencio. |
| Mensaje alterado, repetido o con otro identificador | Se rechaza y **no se guarda nada** del ratchet | Un mensaje falso no puede gastar claves ni desincronizar la sesión (fallo encontrado por las pruebas y corregido). |
| Sesión perdida | El receptor responde `NoSession`; el emisor la olvida y la rehace en el siguiente intento | Se recupera solo. |
| Base de datos local | Texto de los mensajes con **ChaCha20-Poly1305** y estado del ratchet con el cifrado de vodozemac, con claves derivadas de la clave del dispositivo (protegida por Keystore). `secure_delete` activado | Una copia del archivo no revela el texto. |

## Probado

- 18 pruebas automáticas: claves distintas por mensaje, avance del ratchet, rechazo de mensajes alterados, repetidos o con otro identificador, detección de cambio de clave, base de datos copiada ilegible, sesión perdida que se rehace sola.
- En los teléfonos reales: al actualizar, los mensajes que había en claro se cifraron (antes se leía «respuesta al nubia» dentro del archivo; después, nada) y la app los sigue mostrando. Dos rondas de mensajes entre el PC y cada teléfono: 8 de 8 entregados, una sesión de ratchet por dispositivo y nada legible en la base de datos del PC.

## Lo que todavía no cubre

- **Metadatos:** quién habla con quién, cuándo y cuánto ocupa cada mensaje siguen siendo visibles para la red (Fase 9).
- **Teléfono robado y desbloqueado:** quien lo tenga desbloqueado puede leer lo que muestre la app. Bloqueado, Keystore no entrega la clave.
- **Revocar** un dispositivo robado (Fase 10).
- Los nombres y direcciones de los contactos no van cifrados en la base de datos.
- Verificación manual de claves («número de seguridad») entre personas: hoy la confianza nace al escanear la invitación en persona.
