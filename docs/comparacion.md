# MobileMesh frente a Firebase, Supabase y SQLite (comparación honesta)

No hay una herramienta mejor para todo. Esta página dice en qué gana MobileMesh y en qué **no**.

## En una frase

- **SQLite**: guarda datos en el dispositivo. Nada más (ni cifrado, ni sincronización, ni cuentas).
- **Firebase** y **Supabase**: los datos viven en **su** servidor. Funcionan sin conexión con una copia local, pero el original está en la nube.
- **MobileMesh**: los datos viven **en los dispositivos**, cifrados, y se sincronizan entre ellos. No hay servidor con los datos de todos.

MobileMesh usa SQLite por dentro: no compite con él, le añade lo que le falta.

## Comparación

| | MobileMesh | Firebase | Supabase | SQLite |
|---|---|---|---|---|
| Dónde viven los datos | En los dispositivos | Servidores de Google | Servidor PostgreSQL (suyo o tuyo) | En el dispositivo |
| ¿Quién puede leerlos? | Solo los dispositivos de la cuenta y quien tenga permiso | Google (y tu servidor) | Quien controle el servidor | Quien tenga el dispositivo |
| Cifrado de extremo a extremo | ✅ Sí | ❌ No | ❌ No | ❌ No (hay extensiones de pago) |
| Funciona sin internet | ✅ Sí, siempre | ✅ Con caché local | 🟡 Con trabajo extra | ✅ Sí |
| Sincronizar entre dispositivos | ✅ Sin servidor de datos | ✅ Por su servidor | ✅ Por su servidor | ❌ No |
| Cuentas de usuario | ✅ Por clave, sin contraseñas en servidores | ✅ Muy completo (correo, Google, teléfono…) | ✅ Muy completo | ❌ No |
| Entrar con correo o Google | ❌ No | ✅ Sí | ✅ Sí | ❌ No |
| Consultas sobre los datos de **todos** los usuarios (rankings, estadísticas) | ❌ No (cada uno tiene los suyos) | ✅ Sí | ✅ Sí (SQL completo) | — |
| SQL completo (joins, vistas…) | ❌ No: colecciones y campos | ❌ No | ✅ Sí | ✅ Sí |
| Panel web de administración | ❌ Todavía no | ✅ Sí | ✅ Sí | ❌ No |
| Plataformas | Android, web | Android, iPhone, web y más | Todas | Todas |
| Coste | Gratis (los relés los pone la comunidad) | Gratis con límites; luego por uso | Gratis con límites; luego mensual | Gratis |
| Si la empresa cierra o cambia precios | No afecta: no hay empresa en medio | Dependes de Google | Puedes alojarlo tú | No afecta |
| Madurez | 🟡 Proyecto joven | ✅ Años, millones de apps | ✅ Años, muy usado | ✅ El más usado del mundo |

## Cuándo elegir MobileMesh

- Apps **privadas entre personas**: notas, chats, agendas, finanzas personales, salud, diarios, listas compartidas.
- Cuando **no quieres guardar datos de tus usuarios** (ni responder por ellos si se filtran).
- Cuando la app debe **funcionar sin internet** de verdad.
- Cuando no quieres facturas que crecen con los usuarios.

## Cuándo NO elegir MobileMesh (todavía)

- Necesitas **datos centrales que todos consultan**: catálogo de una tienda, ranking global, red social pública. → Supabase o Firebase (o MobileMesh + un «ayudante» en tu servidor para esa parte).
- Necesitas **iPhone nativo** hoy.
- Necesitas **entrar con correo o Google** desde el primer día.
- Necesitas un **panel web** para gestionar datos.

## Las cifras

Ver [rendimiento.md](rendimiento.md): todo medido y repetible con un comando.
