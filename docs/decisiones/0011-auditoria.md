# Informe de la Fase 13: seguridad, auditoría y preparación del lanzamiento

Fecha: 13-10-2026

## Resumen

Una revisión independiente del núcleo no encontró **ninguna forma de leer los datos ni de romper
el cifrado**: la base criptográfica (Noise XX con certificado, Double Ratchet, ChaCha20-Poly1305
con nonces únicos, Argon2id) es sólida. Sí encontró 26 problemas de robustez y permisos, que se
arreglaron casi todos y se volvieron a verificar con una segunda revisión.

## Hallazgos y estado

| # | Gravedad | Problema | Estado |
|---|---|---|---|
| 1 | Alta | Un contacto con permiso de escritura podía falsificar el dispositivo y el número de una operación (bloquear al dueño o hacer perder cambios) | **Arreglado**: dispositivo ligado a su cuenta; de un contacto solo se aceptan dispositivos ya ligados o el que está hablando; números acotados. Prueba de ataque incluida |
| 2 | Alta | Reloj «envenenado» (`u64::MAX`) para ganar siempre y desbordar el reloj propio | **Arreglado**: reloj acotado a +1 día y suma saturada |
| 3 | Alta | Un solo cliente ocupaba todas las conexiones mandando un byte cada 9 s | **Arreglado**: plazos totales (15 s el saludo, 30 s cada paquete), una conexión por hilo, 16 a la vez, 4 por IP y 8 para el relé |
| 4 | Alta | Relé sin límites: hilos, buzones y llamadas sin tope | **Arreglado**: límites globales, por IP, buzones por IP y llamadas por buzón; plazos de silencio e inactividad |
| 5 | Media | El secreto del buzón viajaba en claro al relé | **Arreglado**: registro con reto firmado (Ed25519); el secreto nunca viaja |
| 6 | Media | El relé escribía con el candado puesto (un cliente podía bloquearlo entero) | **Arreglado** |
| 7 | Media | Un dispositivo retirado conserva acceso mientras otro propio no conozca la lista | **Limitación**: al retirar se avisa en el momento y se indica a cuántos no se llegó |
| 8 | Media | Sin cuotas de disco ni de frecuencia para copias y sincronizaciones | **Arreglado en parte**: límites por minuto, una sola copia a medias por dueño, tope de cambios por sincronización. Falta una cuota de disco por contacto |
| 9 | Media | Metadatos de la base de datos sin cifrar (nombres de contactos, colecciones, estados) | **Limitación documentada**: textos, valores, claves y ratchet sí van cifrados. Cifrar el archivo entero (SQLCipher) queda para después |
| 10 | Media | Un canal cifrado podía quedar roto para siempre | **Arreglado**: se pide abrir sesión nueva |
| 11 | Media | Un texto de relleno podía enviarse como mensaje | **Arreglado** |
| 12–26 | Bajas | Identificadores sin validar, hexadecimal con mayúsculas, secretos en `Debug`, límites de claves, orden de comprobaciones, transacciones, migraciones simultáneas, Argon2 mínimo, tabla de límites sin purga, lista de retirados sin tope… | **Arreglados**, salvo los que solo se pueden documentar: el que escucha revela su cuenta a quien conecta (propiedad de Noise XX) y que alguien que guarda una copia pueda devolver una versión antigua |

## Pruebas

- **52 pruebas automáticas**, que pasan siempre (comprobado varias veces seguidas), entre ellas
  9 ataques simulados: basura en el saludo, longitudes enormes, conexiones a medias, desconocidos,
  líneas raras y avalanchas contra el relé, robo de buzón sin la clave, invitaciones y códigos
  malformados, una conexión lenta que no bloquea a las demás, una dirección que no acapara las
  conexiones, y operaciones y reloj falsificados.
- **`cargo audit`**: ninguna vulnerabilidad conocida en las 201 librerías.
- **Licencias**: todas libres y permisivas ([dependencias.md](../dependencias.md)).
- **Teléfono real** (nubia) con la versión final: mensajes en los dos sentidos con el PC.

## Mediciones

| Qué | Resultado |
|---|---|
| Abrir conexión y enviar 1 mensaje (saludo, listas, ratchet) | 3,5 KB |
| Cada mensaje corto más en la misma conexión | 1,1 KB (relleno a 512 bytes incluido) |
| Espacio en el teléfono por mensaje | 400 bytes |
| Archivo de 1 MB por la red | 1,35 MB (base64 + sellos) |
| Sincronización sin cambios | 3,5 KB |
| Procesador en espera (nubia, app en segundo plano) | **0,09 % de un núcleo** (era 1,4 %: la pantalla se refrescaba aunque no se viera; corregido) |
| Datos en espera | Corregido: sincronizar vacío cada 15 s eran ~20 MB/día por dispositivo; ahora solo al cambiar algo o cada 10 min (~0,5 MB/día) |
| Coste del relé | Reenvía lo mismo que pasa por él (3,5 KB por conexión + ~1,1 KB por mensaje); no guarda nada |

## Antes de recomendarlo para información crítica

1. Una **auditoría externa profesional** del protocolo y de la implementación.
2. **Cifrar el archivo entero de la base de datos** (SQLCipher) y una cuota de disco por contacto.
3. Recibir con la **pantalla apagada** sin congelación del fabricante (Fase 11 pendiente).
4. Elegir y publicar la **licencia del proyecto**.
