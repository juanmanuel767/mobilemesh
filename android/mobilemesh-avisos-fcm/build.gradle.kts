// Pieza OPCIONAL: avisos al instante por Firebase (Google). El resto de MobileMesh no depende de ella.
plugins {
    id("com.android.library")
}

android {
    namespace = "org.mobilemesh.avisosfcm"
    compileSdk = 36
    defaultConfig { minSdk = 28 }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    api(project(":mobilemesh-sdk"))
    implementation("com.google.firebase:firebase-messaging:24.1.0")
}
