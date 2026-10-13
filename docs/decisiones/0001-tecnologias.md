# Decisión 0001: tecnologías del núcleo

Fecha: 12-10-2026 · Estado: aceptada

## Decisión

| Área | Elección | Por qué |
|---|---|---|
| Lenguaje del núcleo | **Rust**, compartido | Un solo núcleo para Android, iPhone, PC y navegador (WebAssembly). Android lo usará desde Kotlin. |
| Base de datos | SQLite (`rusqlite`, incluida en el programa) | Funciona igual en todas las plataformas, sin servidor. |
| Conexión y cifrado | Protocolo **Noise XX** (`snow`) sobre TCP | Protocolo público y estudiado (lo usan WireGuard y WhatsApp); autentica a los dos lados y renueva claves en cada sesión. No inventamos criptografía. |
| Identidad | Par de claves X25519 por dispositivo | La clave pública es el identificador; la privada no sale nunca. |
| Formato de mensajes | JSON dentro del canal cifrado | Fácil de leer y depurar; la versión del protocolo va en el saludo Noise. |

## Pendiente de comprobar (no decidido aún)

- **Atravesar routers (NAT) por Internet:** comparar WebRTC y libp2p con dos teléfonos en redes distintas. Hoy solo hay conexión directa por IP (misma red).
- **Cifrado por mensaje** (Double Ratchet) para mensajes guardados por terceros: elegir una implementación auditada (p. ej. `vodozemac` o libsignal) revisando licencias.
- **Puente a Android:** UniFFI para generar el código Kotlin.
- **Sincronización (CRDT):** evaluar en la Fase 7.
