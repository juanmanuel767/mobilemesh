# Decisión 0004: fotos, vídeos y documentos (Fase 6)

Fecha: 12-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Cifrado | Cada archivo con **su propia clave aleatoria** (ChaCha20-Poly1305) por trozos de 32 KB. El número de trozo va en el nonce y en los datos autenticados | Como Signal y Matrix: el archivo cifrado puede guardarlo cualquiera (un relé, otro teléfono) sin poder leerlo. No se pueden reordenar, mezclar ni cortar trozos sin que se note. |
| La clave del archivo | Viaja con el nombre, el tamaño y la huella SHA-256 **dentro del mensaje cifrado con el ratchet** | Solo el destinatario puede abrirlo. |
| Comprobación | Cada trozo se comprueba al llegar; al final se compara la **huella SHA-256** del archivo entero. Si algo falla, se descarta y no se confirma | Criterio de la fase: no se aceptan archivos con integridad incorrecta. |
| Cortes de conexión | El receptor dice cuántos trozos tiene (`Have`) y el emisor **continúa desde ahí** | Un vídeo grande no vuelve a empezar desde cero. |
| Duplicados | Si el destino ya tiene el archivo completo, confirma sin recibirlo otra vez | Reintentos seguros. |
| Copiar, no mover | El remitente guarda su propia copia cifrada; el original no se toca | Así puede reintentar sin conexión y nunca se pierde la foto. |
| Almacenamiento | Los dos guardan el archivo **cifrado** en la carpeta privada de la app. Para verlo se descifra en memoria (la copia temporal se borra al momento); para sacarlo, «Guardar en Descargas» | Una copia del teléfono no revela las fotos; solo salen si el usuario lo pide. |
| Codificación | Base64 dentro del canal (antes hexadecimal, que duplicaba el tráfico) | Encontrado midiendo en las pruebas. |
| Tamaño máximo | 200 MB | Límite provisional. |

## Probado

- 23 pruebas automáticas: archivos de 0 B a 2 MB llegan idénticos; un corte a mitad continúa y solo envía lo que falta; un archivo alterado se rechaza y no se da por recibido; un archivo ya recibido no se reenvía.
- En los teléfonos reales:
  - Foto de 1,7 MB del PC al TECNO en 3,3 s: se ve en la app y, guardada en Descargas, tiene **la misma huella SHA-256** que la original. Dentro de la app solo hay el archivo cifrado.
  - Vídeo de 20 MB del TECNO al PC con el receptor apagado al 17 %: el TECNO lo marcó «enviando 20 %» y pendiente; al reintentar continuó y llegó con **la misma huella**.
  - Encontrado y corregido: con archivos grandes el emisor dejaba de esperar mientras el destino comprobaba la huella; ahora espera 10 s más 1 s por MB.

## Pendiente

- Recibir con la app cerrada o en segundo plano (Fase 11).
- Enviar a la vez a varios dispositivos de una cuenta (Fase 7).
- Compartir desde la galería u otras apps hacia MobileMesh.
- Abrir vídeos y documentos dentro de la app (hoy: vista previa de fotos y «Guardar en Descargas», que necesita Android 10 o superior).
