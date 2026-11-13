#!/bin/sh
# Compila el núcleo para navegador (WebAssembly) y genera web/pkg. Necesita:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.129 --locked
# Prueba: python3 servir.py 8091 → http://127.0.0.1:8091/prueba.html?rele=181.60.197.125:7900
set -eu
DIR_INICIAL="$PWD"
cd "$(dirname "$0")/../core"
cargo build --release --lib --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir ../web/pkg target/wasm32-unknown-unknown/release/mobilemesh_core.wasm
# Deja la carpeta lista para «cd web && npm publish»: el package.json ya está en web/ y
# su lista «files» recoge el SDK, el trabajador y pkg/. Se comprueba que no falte nada.
cd ../web
for f in mobilemesh.js red.js nucleo-worker.js pkg/mobilemesh_core.js pkg/mobilemesh_core_bg.wasm; do
  [ -f "$f" ] || { echo "Falta web/$f" >&2; exit 1; }
done
echo "Paquete web listo en web/pkg"
echo "Para npm: cd web && npm pack --dry-run   (publicar: npm publish)"
# Archivo de la release de GitHub: mobilemesh-web.tar.gz, con una carpeta «mobilemesh/» en la raíz
# (el README lo instala con «curl … | tar xz» e importa ./mobilemesh/mobilemesh.js).
# Se deja en el directorio indicado por la variable SALIDA o, por defecto, en el actual.
SALIDA="${SALIDA:-$DIR_INICIAL}"
mkdir -p "$SALIDA"
TMP_TAR="$(mktemp -d)"
mkdir "$TMP_TAR/mobilemesh"
cp mobilemesh.js red.js nucleo-worker.js servir.py "$TMP_TAR/mobilemesh/"
cp -R pkg "$TMP_TAR/mobilemesh/pkg"
tar czf "$SALIDA/mobilemesh-web.tar.gz" -C "$TMP_TAR" mobilemesh
rm -rf "$TMP_TAR"
echo "Release web: $SALIDA/mobilemesh-web.tar.gz"
