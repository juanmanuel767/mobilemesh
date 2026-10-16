#!/bin/sh
# Compila el núcleo para navegador (WebAssembly) y genera web/pkg. Necesita:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.129 --locked
# Prueba: python3 servir.py 8091 → http://127.0.0.1:8091/prueba.html?rele=181.60.197.125:7900
set -eu
cd "$(dirname "$0")/../core"
cargo build --release --lib --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir ../web/pkg target/wasm32-unknown-unknown/release/mobilemesh_core.wasm
echo "Paquete web listo en web/pkg"
