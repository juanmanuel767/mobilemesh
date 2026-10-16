# Decisión 0014: auditoría de las piezas nuevas

Fecha: 15-10-2026 · Estado: aceptada

Revisadas: guardián, directorio, relé (límites, QUIEN, DIRECTORIO, velocidad), puente WebSocket,
candado TLS, UPnP, núcleo web, servidor MCP y app «Relé MobileMesh».

## Corregido

| Fallo | Riesgo | Arreglo | Prueba |
|---|---|---|---|
| El guardián aceptaba mensajes y archivos (hasta 200 MB) de cualquiera | Llenarle el disco creando cuentas sin parar | Solo atiende copias y códigos de cuenta | `un_desconocido_no_puede_llenar_el_guardian_con_mensajes` |
| Sin tope total para copias de desconocidos | Llenar el disco con muchas cuentas de 50 MB | Tope de 2 GB (`guardian::MAX_TOTAL`) | idem (cuenta el espacio público) |
| Guardar códigos sin límite global | Llenar la base con códigos | 60 por minuto en total | — |
| El relé y el puente veían a todos los usuarios web como 127.0.0.1 | 1000 conexiones bloqueaban la web entera | Las locales no cuentan; el puente limita por la IP real | — |
| El saludo WebSocket solo tenía plazo por byte | Retener conexiones horas mandando un byte cada 9 s | Plazo total de 10 s | — |
| El servidor MCP aceptaba cualquier carpeta | Una IA podía crear archivos en cualquier sitio del equipo | Solo nombres simples dentro de `~/.mobilemesh-mcp` (`MESH_MCP_DIR`) | probado a mano: `/etc/…` y `../..` rechazados |

## Riesgos aceptados (conocidos)

- **Guardián donado malicioso:** quien guarde códigos puede intentar adivinar contraseñas sin
  límite en su copia (cada intento cuesta 64 MiB de Argon2id). Por eso el directorio lo cura el
  proyecto y se exigen 12 caracteres. Con contraseñas débiles, la cuenta corre peligro.
- **Límite global de consultas al guardián (60/min):** frena a quien prueba contraseñas, pero
  alguien puede gastarlo y retrasar las recuperaciones legítimas un rato.
- **Buzón de cartas del relé (64 MB en memoria):** se puede llenar y rechazar cartas nuevas hasta
  que caduquen (7 días). Venía de antes.
- **Límite de velocidad del relé:** quien lo gaste entero hace lento el relé para todos.
- **UPnP en las apps:** abren su puerto en el router de casa; el puerto solo acepta a quien
  demuestre su clave (Noise), pero queda expuesto a intentos de conexión.
- **Web:** la clave de la cuenta va sin envolver en OPFS; un fallo XSS en la página que use el SDK
  la expone. Pendiente envolverla con WebCrypto.
- **App del relé en versión de depuración:** con el móvil conectado, `adb run-as` puede leer el token
  de DuckDNS y la clave del certificado. Para producción, versión «release».
