plugins {
    id("com.android.application")
}

android {
    namespace = "org.mobilemesh.notas"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.mobilemesh.notas"
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
    // Lo único que necesita una app para usar MobileMesh (ni Firebase ni Supabase).
    implementation(project(":mobilemesh-sdk"))
}
