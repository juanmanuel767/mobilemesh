plugins {
    id("com.android.library")
}

android {
    namespace = "org.mobilemesh.sdk"
    compileSdk = 36

    defaultConfig {
        minSdk = 28 // setUnlockedDeviceRequired de Keystore
        // ponytail: solo arm64 (casi todos los móviles actuales); añadir armeabi-v7a si hace falta.
        ndk { abiFilters += "arm64-v8a" }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // UniFFI usa JNA para llamar al núcleo en Rust. «api»: las apps que usan la librería la reciben también.
    api("net.java.dev.jna:jna:5.17.0@aar")
}
