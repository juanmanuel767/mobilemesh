# Decisión 0012: guardián para recuperar cuenta y datos

Fecha: 15-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Pieza | Aparte del relé: módulo `core/src/guardian.rs` y comando `mesh <carpeta> guardian`. En el nubia corre como **otro proceso** de la app «Relé MobileMesh», con su carpeta (`files/guardian`), su puerto (**7954**) y su registro (`guardian.log`) | Si se rompe uno, el otro sigue; se puede mover a otra máquina sin tocar el relé. |
| Qué guarda | El **código de recuperación** (`MM3-`, ya cifrado con la contraseña) y la **copia cifrada de datos** de la Fase 8, para cualquiera | Recuperarse tras perder el teléfono sin depender de un amigo. |
| Cómo se encuentra el código | Localizador = huella SHA-256 de Argon2id(contraseña, sal = usuario); el guardián nunca ve usuario ni contraseña | Solo quien sabe los dos lo encuentra; cada intento cuesta 64 MiB y tiempo. |
| Contraseña mínima | 12 caracteres (10 para el código en papel) | Cualquiera puede probar contraseñas contra el guardián. |
| Límites | 5 intentos por minuto por conexión y 60 en total; 50 MB de copia por cuenta; solo el dueño puede cambiar su código | Frenar a quien adivine y a quien llene el disco. |
| Copias abandonadas | El guardián borra cada 6 h las copias de datos sin señales del dueño en **90 días** (las de amigos con consentimiento, nunca). Las apps comprueban su copia una vez al día, y eso cuenta como señal. Los códigos de cuenta no caducan | El disco no se llena de copias de gente que dejó la app; el código hace falta justo cuando se pierde el teléfono. |
| En las apps | `protegerConGuardian(usuario, contraseña)` y `MobileMesh.recuperarConGuardian(context, usuario, contraseña)` | Dos llamadas: proteger y recuperar. |

## Lo que el guardián SÍ ve

Direcciones IP, horas, tamaño de las copias y cuántas veces se consulta cada localizador. No puede leer códigos ni copias.

## Probado

- Prueba automática completa: guardar código y copia, contraseña equivocada rechazada, otra cuenta no puede reemplazar el código, recuperar cuenta y datos en un teléfono nuevo, el guardián no ve el contenido.

## Pendiente

- Lista `guardian::UNIVERSALES` vacía hasta tener la invitación del guardián del nubia.
- Si el nubia se pierde, las copias se pierden: conviene un segundo guardián (o guardar también en contactos, Fase 8).
