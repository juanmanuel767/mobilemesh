#!/usr/bin/env bash
# Compila el núcleo Rust para el teléfono, genera el código Kotlin y construye la app.
# Uso: android/build.sh [instalar]
set -euo pipefail
cd "$(dirname "$0")/.."

NDK="${ANDROID_NDK_HOME:-$HOME/Android/Sdk/ndk/28.2.13676358}"
T="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
export CC_aarch64_linux_android="$T/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$T/llvm-ar"
export RANLIB_aarch64_linux_android="$T/llvm-ranlib"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$T/aarch64-linux-android26-clang"
export CC_x86_64_linux_android="$T/x86_64-linux-android26-clang"
export AR_x86_64_linux_android="$T/llvm-ar"
export RANLIB_x86_64_linux_android="$T/llvm-ranlib"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$T/x86_64-linux-android26-clang"

# arm64-v8a: móviles reales; x86_64: emulador de Android Studio.
for par in aarch64-linux-android:arm64-v8a x86_64-linux-android:x86_64; do
  TARGET="${par%%:*}"
  ABI="${par##*:}"
  (cd core && cargo build --release --lib --target "$TARGET")
  mkdir -p "android/mobilemesh-sdk/src/main/jniLibs/$ABI"
  cp "core/target/$TARGET/release/libmobilemesh_core.so" "android/mobilemesh-sdk/src/main/jniLibs/$ABI/"
  # Quitar los símbolos de depuración: 11 MB -> 8,9 MB por arquitectura. El SDK
  # va dentro de apps ajenas y ahora lleva dos .so, así que son 4 MB de menos.
  # La copia con símbolos se queda en core/target/ para depurar un fallo nativo.
  "$T/llvm-strip" "android/mobilemesh-sdk/src/main/jniLibs/$ABI/libmobilemesh_core.so"
done
SO=core/target/aarch64-linux-android/release/libmobilemesh_core.so
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
