#!/bin/sh
# Compila el núcleo para navegador (WebAssembly) y genera web/pkg. Necesita:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.129 --locked
# Prueba: python3 servir.py 8091 → http://127.0.0.1:8091/prueba.html?rele=181.60.197.125:7900
set -eu
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
