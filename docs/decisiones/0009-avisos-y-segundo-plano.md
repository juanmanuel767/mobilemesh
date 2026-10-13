# Decisión 0009: recibir con la app cerrada y avisos (Fase 11)

Fecha: 12-10-2026 · Estado: en curso

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Dos canales | **Datos** por la conexión directa o el relé (como hasta ahora) y **avisos** locales que nunca llevan el contenido | Lo que pide el plan. |
| Seguir conectado sin la app abierta | **Servicio en primer plano** («remoteMessaging») con notificación fija: mantiene la escucha directa, el buzón en el relé, los reintentos y la sincronización | Funciona sin Google ni servidores, como Signal en teléfonos sin servicios de Google. |
| Aviso de mensaje nuevo | Notificación local «Mensaje nuevo — De X»; en la pantalla de bloqueo solo «Tienes un mensaje nuevo» | No sale contenido ni a la pantalla de bloqueo ni a ningún servidor. |
| Teléfono reiniciado | El servicio arranca al encender y espera al primer desbloqueo (la identidad solo se abre desbloqueado); después sigue aunque se bloquee | La clave de Keystore exige desbloqueo; una vez abierta queda en memoria. |
| Ahorro de batería | Botón en la app para pedir la exención de optimización de batería | Necesario en casi todos los teléfonos. |
| Avisos de Google (FCM) / Apple (APNs) | **No hechos todavía**: necesitan un proyecto de Firebase y un servidor que envíe los avisos, con cuenta del usuario | El plan los marca como adaptador opcional. |

## Probado en el TECNO KJ5 (HiOS 13.5, Android 13)

| Situación | ¿Llega? | ¿Aviso sin contenido? |
|---|---|---|
| App abierta | Sí | Sí |
| App en segundo plano o cerrada, recién salida | Sí | Sí |
| Escrito sin wifi en el TECNO | Sí: salió solo al volver la conexión | — |
| 1–3 minutos en segundo plano o con la pantalla apagada | **No** | — |

**Causa:** el sistema de Transsion (HiOS) congela el proceso (hilos en `__refrigerator`) aunque Android lo considera un servicio en primer plano activo y no congelado. Probado sin éxito: batería «Sin restricciones», «Inicio automático: Permitido» en Phone Master, app bloqueada en recientes y notificación fija visible. Las apps que no congela (WhatsApp, Telegram, Messenger) están en su lista de fábrica y usan los avisos de Google.

## Probado en el nubia Z2465N (ZTE, Android 16)

| Situación | ¿Llega? |
|---|---|
| App abierta | Sí (y entregó de golpe los 7 mensajes que tenía pendientes) |
| 3 minutos en segundo plano, pantalla encendida | **Sí** (el nubia no la congela mientras la pantalla está encendida) |
| Pantalla apagada (3 y 8 minutos) | **No** |

Con la pantalla apagada el sistema congela el proceso (`do_freezer_trap`) aunque Android lo marca como servicio en primer plano, la red no está bloqueada y la app tiene la exención de batería. Mantener el procesador despierto (bloqueo parcial, ya añadido al servicio) no lo evita.

**Conclusión:** en los dos teléfonos reales, de marcas distintas, un servicio en primer plano no basta para recibir con la pantalla apagada. Lo que sí despiertan estos fabricantes son los avisos de alta prioridad de Google (FCM). Eso convierte el adaptador FCM en necesario en la práctica, no solo opcional.

## Siguiente paso

1. Adaptador de avisos de Google (FCM): el relé envía un aviso «tienes algo pendiente», sin contenido, a quien no está conectado; el aviso de alta prioridad despierta la app, que entonces recoge el mensaje por el canal cifrado. Necesita el proyecto de Firebase del usuario.
2. Alternativa sin Google: UnifiedPush (el usuario instala una app distribuidora como ntfy).
