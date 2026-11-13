plugins {
    id("com.android.library")
}

android {
    namespace = "org.mobilemesh.sdk"
    compileSdk = 36

    defaultConfig {
        minSdk = 28 // setUnlockedDeviceRequired de Keystore
        // ponytail: arm64 (móviles actuales) y x86_64 (emulador); añadir armeabi-v7a si hace falta.
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        // Viajan dentro del .aar: así R8 no renombra las clases que JNA busca
        // por nombre y las apps funcionan en release sin configurar nada.
        consumerProguardFiles("consumer-rules.pro")
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
