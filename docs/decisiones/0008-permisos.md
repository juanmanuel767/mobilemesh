# Decisión 0008: permisos y control de acceso (Fase 10)

Fecha: 12-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Retirar un dispositivo (perdido o robado) | El principal firma con la clave de la cuenta una **lista de retirados** con número de versión. **Al empezar cada conexión** los dos lados se mandan la lista de su cuenta; quien recibe una versión más nueva la guarda y rechaza a los retirados | Se enteran todos los que hablan con cualquier dispositivo válido de la cuenta. Al retirar, el teléfono avisa en el momento a contactos y dispositivos conectados. |
| Quién puede retirar | Solo el principal (tiene la clave de la cuenta); no se puede retirar el dispositivo que se usa | Un dispositivo robado no puede retirar a los buenos. |
| Bloquear | Un contacto bloqueado no puede conectar ni enviar; tampoco se le envía por error | Paso 10 del plan. |
| Límites contra el spam | Como mucho 60 mensajes por minuto de cada contacto (lo que pasa del límite queda pendiente en el remitente, no se pierde) y 30 conexiones por minuto desde cada IP, antes del saludo | Frena inundaciones sin castigar el uso normal. |
| Compartir datos | Una colección se comparte con un contacto **solo para leer** o **para leer y cambiar**. Solo se le envía lo compartido; de lo suyo solo se acepta lo que puede cambiar **y** está a su nombre | Cada cambio queda comprobado: permiso y autoría. |
| Autoría | Cada cambio lleva la cuenta autora; por el canal autenticado se sabe quién lo manda, y un contacto no puede mandar cambios a nombre de otro | «Operaciones verificadas» del plan. |

## Probado

- 41 pruebas automáticas, entre ellas: un dispositivo retirado es rechazado por los contactos y por los dispositivos de la cuenta; una lista de retirados falsificada no se acepta; un vinculado no puede retirar; un bloqueado no puede enviar; pasado el límite los mensajes esperan sin perderse; la lista compartida solo para leer rechaza cambios, nadie puede firmar a nombre de otro, lo no compartido nunca sale y un contacto sin permiso no recibe nada.
- **En el TECNO con el PC como contacto:**
  - Lista compartida solo para leer: el PC recibió «pan»; su «cerveza» fue **rechazada**. Con permiso de escritura, entró.
  - Bloqueo: el mensaje del PC no entró y quedó pendiente; al desbloquear se entregaron los dos.
  - Retiro: se vinculó un dispositivo temporal del PC a la cuenta del TECNO (sincronizó 11 cambios); se retiró desde el botón y a partir de ahí **el TECNO lo rechaza**; el PC, como contacto, recibió la lista de retirados (versión 1).

## Pendiente y límites

- Quien no haya hablado con ningún dispositivo válido de la cuenta después del retiro todavía no lo sabe (no hay servidor que lo publique). Mejorable guardando la lista en los relés.
- Compartir con varias personas a la vez: cada uno debe sincronizar directamente con el dueño (los cambios de un tercero que pasan por otro se rechazan por autoría).
- Los cambios hechos antes de esta fase no llevan autor y no se pueden compartir.
- Reputación (reseñas, valoraciones): no se ha hecho; cuando una app la necesite, atar cada opinión a una interacción verificable, sin prometer que eso elimina las identidades falsas.
- Avisar en la app cuando se rechaza algo por permisos (hoy solo se cuenta).
