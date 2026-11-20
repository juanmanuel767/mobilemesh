package org.pruebalote.a

import android.app.Activity
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.mobilemesh.sdk.MobileMesh
import org.mobilemesh.sdk.MobileMeshError
import org.mobilemesh.sdk.Permissions

/**
 * App de prueba del aislamiento por lotes: «Lote A». Al integrar MobileMesh, Gradle generó
 * la marca de esta app; dos apps con marcas distintas no pueden comunicarse aunque se pasen la
 * invitación. Esta pantalla enseña la marca, la invitación propia y un chat compartido.
 */
class LoteActivity : Activity() {
    private var dejarDeEscuchar: (() -> Unit)? = null

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        val mesh = try {
            MobileMesh.start(this, port = 7821)
        } catch (e: MobileMeshError) {
            return setContentView(TextView(this).apply { text = e.message; setPadding(40, 120, 40, 40) })
        }
        val chat = mesh.collection("chat")

        val marca = assets.open("mobilemesh.properties").use { e ->
            java.util.Properties().apply { load(e) }.getProperty("appId") ?: packageName
        }

        val estado = TextView(this)
        val lista = TextView(this).apply { textSize = 16f }
        val texto = EditText(this).apply { hint = "Escribe un mensaje" }
        val nombre = EditText(this).apply { hint = "Nombre del otro teléfono" }
        val invitacion = EditText(this).apply { hint = "Pega aquí su invitación" }
        val miInvitacion = TextView(this).apply { setTextIsSelectable(true); textSize = 11f }

        val raiz = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(30, 30, 30, 30)
            addView(TextView(context).apply { text = "Lote A"; textSize = 22f })
            addView(TextView(context).apply { text = "Marca de esta app:\n$marca"; textSize = 12f })
            addView(Button(context).apply {
                text = "Ver mi invitación"
                setOnClickListener {
                    miInvitacion.text = try { mesh.invitation() } catch (e: MobileMeshError) { e.message }
                }
            })
            addView(miInvitacion)
            addView(nombre)
            addView(invitacion)
            addView(Button(context).apply {
                text = "Añadir contacto y compartir chat"
                setOnClickListener {
                    estado.text = try {
                        val n = nombre.text.toString().trim()
                        mesh.addContact(n, invitacion.text.toString().trim())
                        chat.shareWith(n, canChange = true)
                        "OK: contacto «$n» añadido y chat compartido"
                    } catch (e: MobileMeshError) {
                        "RECHAZADO: ${e.message}"
                    }
                }
            })
            addView(estado)
            addView(texto)
            addView(Button(context).apply {
                text = "Enviar"
                setOnClickListener {
                    chat.save("m${System.currentTimeMillis()}", mapOf(
                        "texto" to texto.text.toString(),
                        "de" to android.os.Build.MODEL,
                        "hora" to System.currentTimeMillis()))
                    texto.setText("")
                }
            })
            addView(lista)
        }
        setContentView(ScrollView(this).apply { fitsSystemWindows = true; addView(raiz) })

        dejarDeEscuchar = chat.sortBy("hora", descending = true).subscribe { docs ->
            lista.text = docs.joinToString("\n") { "• ${it.fields["texto"]}  (${it.fields["de"]})" }
                .ifEmpty { "Todavía no hay mensajes" }
        }
    }

    override fun onResume() {
        super.onResume()
        Permissions.request(this)
    }

    override fun onDestroy() {
        dejarDeEscuchar?.invoke()
        super.onDestroy()
    }
}
