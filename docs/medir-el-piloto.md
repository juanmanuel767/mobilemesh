[◄ volver](volver/README.md)

# Medir el piloto

El núcleo puede dejar constancia de lo que hace en un CSV: cuánto tardó cada envío, si acabó en
entrega directa o en el buzón, y con qué supernodo se quedó cada teléfono. Sirve para sostener con
datos lo que de otro modo solo serían estimaciones.

**Viene apagado.** Mientras nadie diga dónde escribir no se crea ningún archivo y no cuesta nada.

## Encenderlo

En el escritorio, con una variable de entorno:

```bash
MOBILEMESH_METRICAS=/ruta/medidas.csv cargo run
```

Desde una app (Kotlin o Swift, por la capa FFI):

```kotlin
mesh.startMetrics("${context.filesDir}/medidas.csv")
mesh.metricsPath()   // dónde está escribiendo, o null
mesh.stopMetrics()   // deja de anotar; el archivo se queda
```

El archivo se crea con la cabecera si no existe y se **añade** al final si ya estaba, así que varias
sesiones de prueba caben en el mismo sitio.

## Qué sale

Una línea por suceso, con estas columnas:

| Columna | Qué es |
|---|---|
| `ts_ms` | Milisegundos desde 1970 |
| `evento` | `envio`, `recogida`, `supernodo` o `rtt` |
| `resultado` | Cómo acabó (ver abajo) |
| `objetivo` | El supernodo, o los 8 primeros caracteres del destinatario |
| `latencia_ms` | Lo que tardó la operación completa |
| `n` | Mensajes entregados, cartas recogidas o supernodos en la lista |
| `bateria` | Nivel 0-100 de este teléfono, tal como lo dijo la app |
| `cargando` | 1 si está enchufado |
| `score` | Puntuación del supernodo (0 a 1) |
| `rtt_ms` | Ida y vuelta medida |
| `nota` | Texto libre; en `supernodo`, cuál ganaba y con cuántos puntos |

Resultados por evento:

| Evento | Resultados posibles |
|---|---|
| `envio` | `directo`, `buzon`, `fallo` |
| `recogida` | `cartas`, `vacio` |
| `supernodo` | `primero`, `mantiene`, `cambia` |
| `rtt` | `contesta`, `callado` |

## Qué **no** sale

Ni mensajes, ni claves, ni nombres de contacto, ni direcciones completas de destinatarios. Del
destinatario van ocho caracteres de su identificador: lo justo para distinguir dos teléfonos en una
misma prueba y no para señalar a una persona.

El archivo es del teléfono. Sacarlo de ahí es decisión de quien hace la prueba, no del núcleo.

## Qué se puede responder con esto

- **Tasa de entrega directa frente a buzón**: contar los `envio` por `resultado`.
- **Latencia de envío**: la mediana de `latencia_ms` en los `envio` con resultado `directo`.
- **Si la selección adaptativa compensa**: cuántos `supernodo` acaban en `cambia` frente a
  `mantiene`, y si el `score` sube tras cada cambio.
- **Calidad del enlace a cada candidato**: la serie de `rtt` por `objetivo` a lo largo del día.

Una medida perdida nunca rompe un envío: si el archivo no se puede escribir, esa línea se pierde y
el núcleo sigue como si nada.
