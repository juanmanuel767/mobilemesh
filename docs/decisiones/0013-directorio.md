# Decisión 0013: directorio firmado de relés y guardianes

Fecha: 15-10-2026 · Estado: aceptada

## Decisión

| Tema | Elección | Por qué |
|---|---|---|
| Pieza | Aparte: `core/src/directorio.rs`, herramienta `mesh-directorio` y `tools/publicar-directorio.sh` | Se cambia sin tocar el relé ni el guardián. |
| Qué es | Lista de relés y guardianes con versión, **firmada** con la clave del proyecto (Ed25519) | Añadir o quitar relés y guardianes sin publicar otra versión de la app. |
| Dónde vive | En el relé: `mesh-rele … --directorio <archivo>` lo entrega con la orden `DIRECTORIO` (se lee en cada petición) | Las apps ya hablan con su relé; no hace falta otro servidor. |
| Cuándo se descarga | Al arrancar la app, en segundo plano; en un teléfono nuevo, antes de buscar la cuenta en el guardián | Siempre la lista más reciente. |
| Seguridad | Solo se acepta con la firma del proyecto y versión mayor que la guardada | Un relé no puede meter direcciones falsas ni volver a una lista vieja. |
| Relés ya elegidos | Se mantienen mientras alguno siga en la lista; si los dos salen, se eligen otros | La ruta que tienen los contactos no cambia sin motivo. |
| Clave secreta | Solo en el PC del proyecto: `~/.config/mobilemesh/directorio.clave` (con copia aparte) | Quien la tenga puede cambiar la lista de todas las apps. |

## Uso

```sh
sh tools/publicar-directorio.sh <versión> "<relé1,relé2>" "<invitación-guardián1,…>"
```

## Probado

- Prueba automática: la lista llega por el relé; una falsificada (aunque tenga versión mayor) se ignora; un byte cambiado rompe la firma.
- Real: versión 1 publicada en el nubia y descargada desde el PC por la dirección pública.

## Pendiente

- Si el relé de casa cambia de IP, las apps no pueden descargar la lista nueva desde él: hace falta un nombre fijo (DuckDNS) o un segundo relé en otro sitio.
