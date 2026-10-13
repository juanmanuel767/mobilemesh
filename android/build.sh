#!/usr/bin/env bash
# Compila el núcleo Rust para el teléfono, genera el código Kotlin y construye la app.
# Uso: android/build.sh [instalar]
set -euo pipefail
cd "$(dirname "$0")/.."

NDK="${ANDROID_NDK_HOME:-$HOME/Android/Sdk/ndk/28.2.13676358}"
T="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
export CC_aarch64_linux_android="$T/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$T/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$T/aarch64-linux-android26-clang"

(cd core && cargo build --release --lib --target aarch64-linux-android)
SO=core/target/aarch64-linux-android/release/libmobilemesh_core.so
mkdir -p android/mobilemesh-sdk/src/main/jniLibs/arm64-v8a
cp "$SO" android/mobilemesh-sdk/src/main/jniLibs/arm64-v8a/
(cd core && cargo run -q --features uniffi/cli --bin uniffi-bindgen -- \
  generate --library "../$SO" --language kotlin --out-dir ../android/mobilemesh-sdk/src/main/java --no-format)

cd android
# Gradle necesita un Java con compilador (javac); en este PC el 17 lo trae.
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
./gradlew -q assembleDebug
echo "APK de ejemplo: examples/notas/build/outputs/apk/debug/ejemplo-notas-debug.apk"
echo "APK: android/app/build/outputs/apk/debug/app-debug.apk"
if [[ "${1:-}" == instalar ]]; then
  adb install -r app/build/outputs/apk/debug/app-debug.apk
  adb shell am start -n org.mobilemesh.demo/.MainActivity
fi
