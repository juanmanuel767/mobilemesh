plugins {
    id("com.android.application")
}

android {
    namespace = "org.mobilemesh.demo"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.mobilemesh.demo"
        minSdk = 28 // setUnlockedDeviceRequired de Keystore
        targetSdk = 36
        versionCode = 1
        versionName = "0.1"
        // ponytail: solo arm64 (casi todos los móviles actuales); añadir armeabi-v7a si hace falta.
        ndk { abiFilters += "arm64-v8a" }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // MobileMesh como librería (Fase 12): el mismo SDK que usaría cualquier otra app.
    implementation(project(":mobilemesh-sdk"))
    // Leer y dibujar códigos QR (invitaciones y copia de recuperación). Código abierto, sin servicios de Google.
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
}
