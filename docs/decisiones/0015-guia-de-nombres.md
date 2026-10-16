# Decisión 0015: guía de nombres y conexión en carrera

Fecha: 16-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Pieza | Aparte del relé y del guardián: `core/src/nombres.rs` y comando `mesh <carpeta> nombres`. Su carpeta, su puerto (**7955** previsto en el nubia) | Que las apps funcionen como cualquier base de datos con usuarios (registrarse, buscar por nombre) sin pasar invitaciones; se cambia sin tocar las otras piezas. |
| Qué guarda | `nombre → invitación` (datos públicos) y las solicitudes pendientes de cada cuenta | Nada privado: mensajes y datos nunca pasan por ella. |
| Quién puede registrar | La cuenta se demuestra en el saludo cifrado y la invitación debe ser de esa cuenta; el primero que registra un nombre se lo queda; una cuenta, un nombre | Nadie puede suplantar ni quitar un nombre. |
| Contactos sin códigos | «Añadir por nombre» deja una solicitud en la guía; la app del otro la recoge y añade al remitente (hasta 50 por cuenta) | Los móviles siguen aceptando solo a sus contactos: no se abre la puerta a desconocidos. |
| Límites | 60 operaciones por minuto por cuenta y 600 en total | Frenar abusos. |
| Conexión | Las rutas «directa\|relé» compiten: la directa sale primero y el relé 400 ms después; gana el primer saludo correcto | Entre redes distintas los mensajes tardaban hasta 10 s; y si la IP privada la contestaba otro dispositivo (otra app de MobileMesh en el mismo puerto) se rechazaba sin probar el relé. |
| React Native | Puente `react-native/` en el mismo repositorio, `npm install github:juanmanuel767/mobilemesh`; puerto propio por app | Un comando; dos apps de MobileMesh en el mismo móvil no se pisan. |

## Lo que la guía SÍ ve

Qué nombres existen, sus invitaciones (direcciones IP y relé), quién pide hablar con quién, IPs y horas.

## Probado

- Línea de comandos: registrar, nombre ocupado rechazado, nombre no válido rechazado, buscar, solicitud aceptada.
- Prueba automática: la ruta suplantada no bloquea la siguiente (`tests/red.rs`).
- Dos móviles reales (TECNO ⇄ Redmi) en la misma wifi y en redes distintas.

## Pendiente

- Publicar la guía del nubia y ponerla en `nombres::UNIVERSALES` (y en el directorio firmado).
- Aviso inmediato de solicitudes (hoy las apps preguntan cada 15 s con la app abierta).
