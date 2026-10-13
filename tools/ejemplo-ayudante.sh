#!/bin/sh
# Ejemplo de funciones del ayudante: $1 = función, entrada = {"de": cuenta, "datos": {...}}
entrada=$(cat)
case "$1" in
  saludar) echo "{\"mensaje\": \"hola, te atiende el ayudante\", \"recibido\": $entrada}" ;;
  sumar) echo "$entrada" | python3 -c 'import json,sys; d=json.load(sys.stdin)["datos"]; print(d["a"]+d["b"])' ;;
  *) echo "no existe la función $1" >&2; exit 1 ;;
esac
