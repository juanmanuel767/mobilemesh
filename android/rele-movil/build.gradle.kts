plugins {
    id("com.android.application")
}

// Convierte un móvil en relé de MobileMesh: arranca solo al encender y no depende de Termux.
android {
    namespace = "org.mobilemesh.rele"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.mobilemesh.rele"
        minSdk = 28
        targetSdk = 36
        versionCode = 1
        versionName = "0.1"
        ndk { abiFilters += "arm64-v8a" }
    }

    // El relé va como «libmeshrele.so» para que Android lo deje en disco y se pueda ejecutar.
    packaging { jniLibs { useLegacyPackaging = true } }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
