package org.mobilemesh.notas

import android.app.Activity
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.mobilemesh.sdk.MobileMesh
import org.mobilemesh.sdk.MobileMeshError
import org.mobilemesh.sdk.Permisos

/**
 * Ejemplo de app hecha con MobileMesh en lugar de Firebase o Supabase: notas que se guardan en el
 * teléfono y se comparten con otra persona, sin servidor. Todo lo de MobileMesh son ~10 líneas.
 */
class NotasActivity : Activity() {
    private var dejarDeEscuchar: (() -> Unit)? = null

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        val mesh = try {
            MobileMesh.iniciar(this, puerto = 7811) // 1. abrir (o crear) la cuenta
        } catch (e: MobileMeshError) {
            return setContentView(TextView(this).apply { text = e.message; setPadding(40, 120, 40, 40) })
        }
        val notas = mesh.coleccion("notas") // 2. una colección

        val estado = TextView(this)
        val lista = TextView(this).apply { textSize = 17f }
        val nota = EditText(this).apply { hint = "Escribe una nota" }
        val buscar = EditText(this).apply { hint = "Buscar en las notas" }
        val nombre = EditText(this).apply { hint = "Nombre de la otra persona" }
        val invitacion = EditText(this).apply { hint = "Pega aquí su invitación" }

        val raiz = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(40, 40, 40, 40)
            addView(TextView(context).apply { text = "Notas compartidas"; textSize = 24f })
            addView(nota)
            addView(Button(context).apply {
                text = "Guardar nota"
                setOnClickListener {
                    // 3. guardar: queda en el teléfono al momento y se comparte solo
                    notas.guardar("n${System.currentTimeMillis()}", mapOf("texto" to nota.text.toString(), "de" to android.os.Build.MODEL, "hora" to System.currentTimeMillis()))
                    nota.setText("")
                }
            })
            addView(buscar)
            addView(lista)
            addView(TextView(context).apply { text = "\nCompartir con otra persona"; textSize = 18f })
            val miInvitacion = TextView(context).apply { setTextIsSelectable(true) }
            addView(Button(context).apply {
                text = "Ver mi invitación"
                setOnClickListener {
                    miInvitacion.text = try {
                        "Tu invitación (pásasela):\n" + mesh.invitacion()
                    } catch (e: MobileMeshError) {
                        e.message
                    }
                }
            })
            addView(miInvitacion)
            addView(nombre)
            addView(invitacion)
            addView(Button(context).apply {
                text = "Compartir mis notas con esta persona"
                setOnClickListener {
                    try {
                        val n = nombre.text.toString().trim()
                        mesh.anadirContacto(n, invitacion.text.toString()) // 4. contacto…
                        notas.compartirCon(n, puedeCambiar = true) // 5. …y compartir
                        estado.text = "Compartido con $n: sus notas y las tuyas se juntarán solas"
                    } catch (e: MobileMeshError) {
                        estado.text = "Error: ${e.message}"
                    }
                }
            })
            addView(estado)
        }
        setContentView(ScrollView(this).apply { fitsSystemWindows = true; addView(raiz) })

        // 6. tiempo real con consulta: las notas que contienen lo buscado, las más nuevas primero;
        // se redibuja cuando cambia algo, aquí o en el teléfono de la otra persona
        fun mostrar(texto: String) {
            dejarDeEscuchar?.invoke()
            dejarDeEscuchar = notas.donde("texto", "contiene", texto).ordenarPor("hora", descendente = true).escuchar { docs ->
                lista.text = docs.joinToString("\n") { "• ${it.campos["texto"]}  (${it.campos["de"]})" }
                    .ifEmpty { if (texto.isEmpty()) "Todavía no hay notas" else "Ninguna nota contiene «$texto»" }
            }
        }
        mostrar("")
        buscar.addTextChangedListener(object : android.text.TextWatcher {
            override fun afterTextChanged(e: android.text.Editable?) = mostrar(e.toString().trim())
            override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
            override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
        })
    }

    override fun onResume() {
        super.onResume()
        Permisos.pedir(this) // 7. permisos para recibir con la app cerrada (solo pregunta lo que falte)
    }

    override fun onDestroy() {
        dejarDeEscuchar?.invoke()
        super.onDestroy()
    }
}
