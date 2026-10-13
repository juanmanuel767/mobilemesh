# Decisión 0005: modo sin conexión y sincronización (Fase 7)

Fecha: 12-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Cambios | Cada cambio es una **operación** numerada por su dispositivo y se guarda primero en local (cifrada) | Funciona sin conexión; nada se pierde si no hay red. |
| Sincronizar | Cada lado manda su **vector de versiones** (hasta qué nº tiene de cada dispositivo) y recibe solo lo que le falta, en tandas, cifrado con el ratchet | Poco tráfico; recibir dos veces o en otro orden da el mismo resultado (probado). |
| Orden de los cambios | **Reloj híbrido** (milisegundos + contador que nunca retrocede y se adelanta al recibir) | No depende de que los relojes de los teléfonos estén en hora (probado con un reloj una hora adelantado). |
| Regla «el último gana» por campo (`Lww`) | Para perfiles y ajustes: cada campo se queda con el cambio más reciente | Cambios en campos distintos nunca se pisan. |
| Regla estricta (`Strict`) | Para saldos, inventario o dinero: dos cambios hechos a la vez sobre el mismo campo **no se mezclan**: se guardan los dos y se avisa hasta que alguien elija. Un conflicto solo lo cierra quien lo tuvo delante | El plan exige que lo que necesita consistencia estricta no dependa de fusiones automáticas. |
| Quién sincroniza | Solo los dispositivos de **la misma cuenta** (se añaden con su invitación); un contacto ajeno es rechazado | Los datos de la cuenta son privados. |
| Reintentos | Cada 15 s se reintentan los mensajes pendientes y se sincroniza; si un destino no responde, se espera el doble cada vez (hasta 5 min) | Recupera solo la conexión sin gastar batería en bucle. |
| Modo sin conexión | Interruptor en la app: no envía, no sincroniza ni acepta conexiones; todo queda en local | Para el usuario y para probar el criterio de la fase. |
| Confirmaciones | Mensajes: pendiente → enviando → entregado (guardado en destino). Datos: «N cambios sin confirmar» hasta que cada dispositivo propio confirma tenerlos | Paso 7.4 del plan. |

## Probado

- 30 pruebas automáticas, entre ellas: perfil combinado campo a campo, saldo en conflicto con los dos valores conservados, recibir repetido o desordenado, reloj adelantado, valores cifrados en disco, 400 cambios en varias tandas, un contacto ajeno no puede sincronizar.
- **Fallo encontrado en el teléfono real y corregido:** una corrección hecha en un teléfono que no había visto el conflicto lo cerraba y el otro valor desaparecía sin aviso. Ahora cada cambio dice qué conflictos tenía delante, y hay una prueba para ese caso.
- **Criterio de la fase, en el TECNO y el nubia (misma cuenta):** saldo 100 sincronizado → los dos en modo sin conexión → el TECNO cambia el nombre y pone el saldo a 80; el nubia cambia el estado y pone otro saldo → al volver la conexión se sincronizan solos (sin pulsar nada) → los dos muestran el perfil combinado (nombre del TECNO y estado del nubia) y el saldo **en conflicto** con los dos valores → se elige 80 en el TECNO y a los pocos segundos los dos muestran 80 sin conflicto.

## Pendiente

- Los **mensajes** aún no se copian entre los dispositivos de una cuenta (cada uno ve los suyos), ni se envían a todos los dispositivos del contacto.
- Documentos colaborativos (texto editado a la vez): necesitarán un CRDT de texto (p. ej. Automerge o Yjs).
- La regla estricta compara con la versión vista; con tres o más dispositivos puede avisar de conflictos que no lo son (nunca al revés: no oculta conflictos reales).
- Los nombres de colecciones, registros y campos se guardan sin cifrar (los valores sí van cifrados).
- Los dispositivos propios confían plenamente entre sí: uno de ellos podría firmar operaciones a nombre de otro.
