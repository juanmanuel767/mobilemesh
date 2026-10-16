#!/bin/sh
# Firma la lista de relés y guardianes y la deja en el relé del móvil (app «Relé MobileMesh»).
# Las apps la descargan solas al arrancar. Sube la versión cada vez que cambies algo.
#
#   sh tools/publicar-directorio.sh <versión> "<relé1,relé2>" "<invitación-guardián1,…>" [serie-adb]
set -eu
V="${1:?versión (número mayor que la anterior)}"
RELES="${2:?relés separados por comas}"
GUARDIANES="${3:-}"
SERIE="${4:-323952058590}"
CLAVE="${CLAVE:-$HOME/.config/mobilemesh/directorio.clave}"
cd "$(dirname "$0")/../core"
LINEA=$(cargo run -q --release --bin mesh-directorio -- firmar "$CLAVE" "$V" "$RELES" "$GUARDIANES")
echo "$LINEA" | adb -s "$SERIE" shell "run-as org.mobilemesh.rele sh -c 'cat > files/directorio.txt'"
echo "Directorio versión $V publicado en el relé."
