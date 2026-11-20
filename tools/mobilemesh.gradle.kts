// Marca de la app («lote») para MobileMesh.
//
// Se genera UNA sola vez, la primera vez que compilas, y queda en
// `src/main/assets/mobilemesh.properties`. Guárdalo en tu repositorio: viaja dentro del APK y es
// la misma en todas las copias que publiques, así que todos tus usuarios se comunican entre sí.
// Otra app tendrá otra marca y no se mezclará con la tuya nunca.
//
// Para usarlo, una línea en el `build.gradle.kts` de tu app:
//
//     apply(from = "mobilemesh.gradle.kts")
//
// Si borras el archivo generado, la próxima compilación crea una marca nueva y tus usuarios
// dejarán de encontrarse entre versiones: no lo borres.

val marcaMobileMesh = file("src/main/assets/mobilemesh.properties")

val generarMarcaMobileMesh = tasks.register("mobilemeshAppId") {
    description = "Genera la marca de esta app para MobileMesh si todavía no existe."
    outputs.file(marcaMobileMesh)
    doLast {
        if (marcaMobileMesh.exists()) return@doLast
        val azar = ByteArray(16).also { java.security.SecureRandom().nextBytes(it) }
        val id = "mm-" + azar.joinToString("") { "%02x".format(it) }
        marcaMobileMesh.parentFile.mkdirs()
        marcaMobileMesh.writeText(
            """
            # Marca de esta app en MobileMesh (su «lote»): solo las copias con esta misma marca
            # se comunican entre sí. Generada automáticamente al integrar; no la cambies ni la
            # borres, y guárdala en el repositorio junto al resto del proyecto.
            appId=$id
            """.trimIndent() + "\n"
        )
        logger.lifecycle("MobileMesh: marca de la app generada → $id")
    }
}

tasks.matching { it.name == "preBuild" }.configureEach { dependsOn(generarMarcaMobileMesh) }
