pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}

plugins {
    id("com.android.application") version "9.0.1" apply false
    id("com.android.library") version "9.0.1" apply false
}

rootProject.name = "mobilemesh"
include(":app")
// La librería que usa cualquier app (Fase 12) y una app de ejemplo independiente.
include(":mobilemesh-sdk")
// Pieza opcional: avisos al instante por Firebase (solo para quien la quiera).
include(":mobilemesh-avisos-fcm")
include(":ejemplo-notas")
project(":ejemplo-notas").projectDir = file("../examples/notas")
