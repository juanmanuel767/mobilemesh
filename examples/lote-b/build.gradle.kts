plugins {
    id("com.android.application")
}

android {
    namespace = "org.pruebalote.b"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.pruebalote.b"
        minSdk = 28
        targetSdk = 36
        versionCode = 1
        versionName = "0.1"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(project(":mobilemesh-sdk"))
}

// Integrar MobileMesh: esta línea genera la marca de la app la primera vez que se compila.
apply(from = "../../tools/mobilemesh.gradle.kts")
