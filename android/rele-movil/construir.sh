#!/bin/sh
# Compila el relé y el guardián para Android y construye la app «Relé MobileMesh».
# Uso: sh android/rele-movil/construir.sh   →   android/rele-movil/build/outputs/apk/debug/rele-movil-debug.apk
set -eu
cd "$(dirname "$0")/../../core"
NDK="${ANDROID_NDK_HOME:-$HOME/Android/Sdk/ndk/28.2.13676358}"
T="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
export CC_aarch64_linux_android="$T/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$T/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$T/aarch64-linux-android26-clang"
cargo build --release --target aarch64-linux-android --bin mesh --bin mesh-rele
L=../android/rele-movil/src/main/jniLibs/arm64-v8a
mkdir -p "$L"
cp target/aarch64-linux-android/release/mesh-rele "$L/libmeshrele.so"
cp target/aarch64-linux-android/release/mesh "$L/libmeshguardian.so"
cd ../android
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
./gradlew -q :rele-movil:assembleDebug
echo "APK: android/rele-movil/build/outputs/apk/debug/rele-movil-debug.apk"
