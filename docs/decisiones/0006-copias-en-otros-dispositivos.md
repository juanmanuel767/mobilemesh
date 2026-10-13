# Decisión 0006: copias cifradas en otros dispositivos (Fase 8)

Fecha: 12-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Qué se copia | Contactos, otros dispositivos de la cuenta, mensajes (texto y datos de los archivos) y datos sincronizados | Es lo que se pierde con el teléfono. |
| Cifrado | La copia se cifra como un archivo (Fase 6) y su clave se **envuelve con una clave sacada de la clave de la cuenta**. La cuenta y la versión van autenticadas | Quien la guarda no puede leerla ni hacer pasar una copia vieja por nueva. Quien recupera la cuenta con su código `MM2-` sí puede abrirla. |
| Voluntario | Un contacto solo guarda tu copia si acepta («Aceptar guardar copias suyas»); dejar de aceptar la borra. Los dispositivos propios la aceptan siempre | Paso 8.1 del plan. |
| Varias copias | Cada guardián tiene estado: prevista, al día, desactualizada, verificada, dañada, sin respuesta, borrando | Paso 8.2: saber qué copias hay, cuáles están bien y cuáles hay que actualizar. |
| Comprobar sin descargar | Se pide la huella SHA-256 de (número al azar + copia): solo se puede calcular teniendo la copia entera | Se detecta una copia dañada o perdida sin moverla por la red. |
| Recuperar | En el teléfono nuevo: recuperar la cuenta con `MM2-`, añadir la invitación de quien guarda la copia y «Recuperar mis datos desde él». Cada trozo y la huella final se comprueban | Paso 8.3. La cuenta se demuestra en el saludo; nadie más puede pedir tu copia. |
| Borrar | Se pide a cada guardián; si no está conectado queda «borrando» y se reintenta en segundo plano hasta que confirme | Paso 8.4: borrado verificable al volver, sin prometer el imposible de borrar lo que está desconectado. |
| Quién hace copias | Solo el dispositivo principal (tiene la clave de la cuenta) | Mínimo privilegio. |

## Probado

- 34 pruebas automáticas, entre ellas: perder el teléfono y recuperar todo desde un amigo; sin permiso no se guarda; un tercero no puede llevarse tu copia; un byte cambiado se detecta sin descargar y al recuperar; una copia vieja no pasa por nueva; el borrado se completa al volver el guardián y no deja restos en disco.
- **En el TECNO real, con el PC como guardián (un tercero):**
  - El PC guarda 5,4 KB que no contienen nada legible; la comprobación sin descarga da «verificada».
  - Se **borraron todos los datos de la app del TECNO** (también su clave de Keystore). Con el código `MM2-` se recuperó la misma cuenta (como dispositivo nuevo) y, desde el PC, **1 contacto, 14 mensajes y 6 cambios de datos**: el perfil (Juan Manuel, de viaje), el saldo (80) y la conversación volvieron.
  - Encontrado y corregido: los mensajes con foto o vídeo recuperados ofrecían «Guardar en Descargas» aunque el archivo no viene en la copia; ahora la app lo dice.
- No se pudo probar con el nubia como guardián: se desconectó de la red durante la prueba.

## Pendiente

- **Fotos y vídeos no van en la copia** (solo su nombre y datos): hará falta copiarlos aparte, por tamaño.
- La copia se hace al pulsar el botón; falta hacerla sola cada cierto tiempo.
- Tras recuperar hay que añadir a mano la invitación de quien guarda la copia (el teléfono nuevo no sabe dónde está). Se podría apuntar en la copia de recuperación.
- Repartir la copia en trozos entre varios guardianes (que ninguno tenga la copia entera).
