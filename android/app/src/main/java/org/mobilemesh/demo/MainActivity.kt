package org.mobilemesh.demo

import android.app.Activity
import android.app.KeyguardManager
import android.content.ContentValues
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.Typeface
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.View
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.Switch
import android.widget.TextView
import com.google.zxing.BarcodeFormat
import com.google.zxing.integration.android.IntentIntegrator
import com.journeyapps.barcodescanner.BarcodeEncoder
import java.io.File
import java.net.Inet4Address
import java.net.NetworkInterface
import kotlin.concurrent.thread
import org.mobilemesh.sdk.KeystoreGuard
import org.mobilemesh.sdk.MeshHolder
import org.mobilemesh.sdk.MeshService
import uniffi.mobilemesh_core.Mesh
import uniffi.mobilemesh_core.MeshException
import uniffi.mobilemesh_core.Message
import uniffi.mobilemesh_core.IdentityState
import uniffi.mobilemesh_core.identityState
import uniffi.mobilemesh_core.linkFingerprint
import uniffi.mobilemesh_core.linkRequest
import uniffi.mobilemesh_core.parseInvite

private const val PICK_FILE = 42

/** App de prueba: un dispositivo MobileMesh con su identidad, copia de recuperación, contactos y mensajes. */
class MainActivity : Activity() {
    private val ui = Handler(Looper.getMainLooper())
    private val guard by lazy { KeystoreGuard(this) }
    // Carpeta privada de la app: otras apps no pueden leerla.
    private val dir by lazy { filesDir.resolve("mesh").path }
    private val prefs by lazy { getPreferences(MODE_PRIVATE) }
    // La identidad abierta se comparte con el servicio que recibe en segundo plano.
    private var mesh: Mesh?
        get() = MeshHolder.mesh
        set(v) { MeshHolder.mesh = v }
    private var inbox: LinearLayout? = null
    private var inboxSig = ""
    private var dataBox: LinearLayout? = null
    private var dataSig = ""
    private var copyBox: LinearLayout? = null
    private var copySig = ""
    private var permBox: LinearLayout? = null
    private var permSig = ""
    private var permName: EditText? = null
    private val thumbs = HashMap<String, Bitmap?>()
    private var onPick: ((Uri) -> Unit)? = null
    private var onScan: ((String) -> Unit)? = null
    private lateinit var status: TextView

    // Solo se refresca la pantalla mientras se ve: en segundo plano no gasta (medido en la Fase 13).
    private var visible = false

    override fun onStart() {
        super.onStart()
        visible = true
        refresh()
    }

    override fun onResume() {
        super.onResume()
        org.mobilemesh.sdk.Permisos.pedir(this) // lo que falte para recibir al 100 %
    }

    override fun onStop() {
        visible = false
        super.onStop()
    }

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        when (identityState(dir, guard)) {
            IdentityState.NOTHING -> return welcome()
            IdentityState.PENDING -> return linkWaitScreen()
            IdentityState.READY -> {}
        }
        try {
            if (mesh == null) mesh = Mesh.open(dir, guard)
        } catch (e: Exception) {
            if (e.text().startsWith("BLOQUEADO")) return lockedScreen()
            return restoreScreen("No se pudo abrir tu identidad: ${e.text()}")
        }
        mainScreen()
    }

    // ---------- Pantallas ----------

    /** Teléfono bloqueado: se espera a que lo desbloqueen y se vuelve a abrir. Nunca se ofrece borrar. */
    private fun lockedScreen() {
        status = TextView(this)
        page(
            title("Teléfono bloqueado"),
            text("Tu identidad está protegida y solo se abre con el teléfono desbloqueado. Desbloquéalo y MobileMesh se abrirá solo."),
        )
        fun waitUnlock() {
            if (getSystemService(KeyguardManager::class.java).isDeviceLocked) ui.postDelayed(::waitUnlock, 1500) else recreate()
        }
        waitUnlock()
    }

    private fun welcome() {
        status = TextView(this)
        page(
            title("Bienvenido a MobileMesh"),
            text("Tus datos se guardan en este teléfono. No hay contraseña en ningún servidor: tu identidad es una clave que solo vive aquí."),
            button("Crear cuenta nueva") {
                work("Creando tu identidad…") {
                    mesh = Mesh.create(dir, guard)
                    ui.post { mainScreen() }
                    null
                }
            },
            button("Vincular a una cuenta que ya tengo en otro teléfono") { linkWaitScreen() },
            button("Ya tengo una copia de recuperación") { restoreScreen(null) },
            status,
        )
    }

    /** Dispositivo nuevo: muestra su solicitud y espera el certificado del principal. */
    private fun linkWaitScreen() {
        status = TextView(this)
        val req = try {
            linkRequest(dir, guard)
        } catch (e: Exception) {
            return restoreScreen("No se pudo preparar la vinculación: ${e.text()}")
        }
        val answer = field("…o pega aquí el certificado")
        fun complete(text: String) = work("Comprobando el certificado…") {
            mesh = Mesh.completeLink(dir, guard, text)
            ui.post { mainScreen() }
            null
        }
        page(
            title("Vincular este teléfono"),
            text("1. En tu teléfono principal abre MobileMesh y pulsa «Vincular otro dispositivo».\n2. Escanea con él este código.\n3. Comprueba que allí aparece el mismo código de comprobación:"),
            TextView(this).apply { text = req.fingerprint; textSize = 28f; typeface = Typeface.MONOSPACE },
            qr(req.text),
            mono(req.text).apply { setTextIsSelectable(true) },
            text("4. Después escanea aquí el certificado que te muestre el principal."),
            button("Escanear certificado") { scan(::complete) },
            answer,
            button("Completar vinculación") { complete(answer.text.toString()) },
            button("Cancelar") {
                File(dir, "identity.key").delete() // solo había una clave de dispositivo sin cuenta
                welcome()
            },
            status,
        )
    }

    /** Dispositivo principal: aprueba la solicitud de otro y le muestra su certificado. */
    private fun approveScreen() {
        val m = mesh!!
        status = TextView(this)
        val request = field("…o pega aquí la solicitud")
        val confirm = Button(this).apply { visibility = View.GONE }
        fun check(text: String) {
            request.setText(text)
            try {
                val code = linkFingerprint(text)
                confirm.text = "Sí, el otro teléfono muestra $code: vincular"
                confirm.visibility = View.VISIBLE
                status.text = "Comprueba que el otro teléfono muestra exactamente: $code"
            } catch (e: Exception) {
                status.text = "Error: ${e.text()}"
            }
        }
        confirm.setOnClickListener {
            val text = request.text.toString()
            work("Firmando…") {
                val cert = m.approveLink(text)
                ui.post { showQr("Certificado para el otro teléfono", "Escanéalo desde el teléfono nuevo. Es público: solo sirve para ese dispositivo.", cert, share = false) }
                null
            }
        }
        page(
            title("Vincular otro dispositivo"),
            text("En el teléfono nuevo elige «Vincular a una cuenta que ya tengo» y escanea aquí el código que muestre."),
            button("Escanear solicitud") { scan(::check) },
            request,
            button("Comprobar solicitud") { check(request.text.toString()) },
            confirm,
            status,
            button("Volver") { mainScreen() },
        )
    }

    private fun restoreScreen(problem: String?) {
        status = TextView(this)
        val code = field("Código de la copia (MM2-…)")
        val pass = field("Contraseña de la copia", secret = true)
        var confirm = false
        page(
            title("Recuperar identidad"),
            problem?.let { warn(it) },
            text("Escanea o pega tu copia de recuperación y escribe su contraseña."),
            button("Escanear QR de la copia") { scan { code.setText(it) } },
            code,
            pass,
            button("Recuperar") {
                val (c, p) = code.text.toString() to pass.text.toString()
                work("Recuperando… (tarda unos segundos)") {
                    mesh = Mesh.restore(dir, guard, c, p)
                    prefs.edit().putBoolean("backup", true).apply() // quien restaura ya tiene su copia
                    ui.post { mainScreen() }
                    null
                }
            },
            if (problem == null) button("Volver") { welcome() } else button("Empezar con una identidad nueva") {
                // Destructivo: se pide una segunda pulsación.
                if (!confirm) {
                    confirm = true
                    status.text = "Pulsa otra vez para confirmar: la identidad dañada se borrará y tus contactos tendrán que añadirte de nuevo."
                    return@button
                }
                File(dir, "identity.key").delete()
                prefs.edit().putBoolean("backup", false).apply()
                welcome()
            },
            status,
        )
    }

    private fun mainScreen() {
        val m = mesh!!
        status = TextView(this)
        val contacts = Spinner(this)
        fun loadContacts() {
            contacts.adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, m.contacts().map { it.name })
        }
        loadContacts()
        val invite = field("…o pega aquí una invitación")
        val name = field("Nombre para este contacto")
        val message = field("Escribe un mensaje")
        val profileName = field("Nombre (perfil)")
        val profileState = field("Estado (perfil)")
        val balance = field("Saldo de la caja").apply { inputType = InputType.TYPE_CLASS_NUMBER }
        val copyName = field("Contacto o dispositivo (para copias)")
        val relayField = field("Relé para otras redes (ip:puerto)")
        val who = field("Contacto (permisos y listas)").also { permName = it }
        val item = field("Algo para la lista")
        fun permOp(label: String, op: (String) -> String) {
            val n = who.text.toString().trim()
            if (n.isEmpty()) status.text = "Escribe el nombre del contacto" else work(label) { op(n).also { ui.post { permSig = "" } } }
        }
        fun addItem(list: () -> String) {
            val t = item.text.toString().trim()
            if (t.isEmpty()) return run { status.text = "Escribe algo para la lista" }
            work("Guardando…") {
                m.put(list(), java.util.UUID.randomUUID().toString(), "texto", t, false)
                ui.post { item.setText(""); permSig = "" }
                "Añadido «$t»"
            }
        }
        fun copyOp(label: String, op: (String) -> String) {
            val n = copyName.text.toString().trim()
            if (n.isEmpty()) status.text = "Escribe el nombre del contacto o dispositivo" else work(label) { op(n).also { ui.post { copySig = "" } } }
        }
        fun send(body: String) {
            val to = contacts.selectedItem as String? ?: return run { status.text = "Primero añade un contacto" }
            message.setText("")
            work("Enviando…") { resultado(m.send(to, body)) }
        }
        onPick = { uri ->
            val to = contacts.selectedItem as String?
            if (to == null) status.text = "Primero añade un contacto" else work("Cifrando y enviando el archivo…") {
                // Se copia a la caché, el núcleo guarda su copia cifrada y la copia en claro se borra enseguida.
                val name = fileName(uri)
                val tmp = File(cacheDir, "salida-${System.nanoTime()}")
                try {
                    contentResolver.openInputStream(uri)!!.use { input -> tmp.outputStream().use { input.copyTo(it) } }
                    val mime = contentResolver.getType(uri) ?: "application/octet-stream"
                    resultado(m.sendFile(to, tmp.path, name, mime))
                } catch (e: Exception) {
                    "No entregado todavía (${e.text()}); queda pendiente"
                } finally {
                    tmp.delete()
                }
            }
        }
        val secure = getSystemService(KeyguardManager::class.java).isDeviceSecure
        val primary = m.isPrimary()
        page(
            title("Mi cuenta"),
            mono(m.id()),
            text("Este dispositivo: ${m.fingerprint()} (${if (primary) "principal" else "vinculado"})"),
            if (secure) null else warn("Este teléfono no tiene bloqueo de pantalla. Activa un PIN, patrón o huella: protege tu identidad."),
            if (!primary || prefs.getBoolean("backup", false)) null else warn("Aún no has creado tu copia de recuperación. Sin ella, si pierdes el teléfono pierdes tu identidad."),
            button("Mostrar mi invitación (QR)") {
                try {
                    showQr("Mi invitación", "Pide a tu contacto que la escanee desde MobileMesh. Solo contiene datos públicos.", m.invite(myAddr()), share = false)
                } catch (e: Exception) {
                    status.text = "Error: ${e.text()}"
                }
            },
            if (primary) button("Crear copia de recuperación") { backupScreen() } else text("La copia de recuperación se crea en el dispositivo principal."),
            if (primary) button("Vincular otro dispositivo") { approveScreen() } else null,
            text("Recibiendo en ${myAddr()}" + (m.relayInfo()?.let { "\ny a través del relé ${it.substringAfter("relay=").substringBefore("/")} (buzón ${it.substringAfterLast("/").take(8)}…)" } ?: "")),
            relayField,
            if (getSystemService(android.os.PowerManager::class.java).isIgnoringBatteryOptimizations(packageName)) {
                text("Funcionamiento en segundo plano: permitido")
            } else {
                button("Permitir que reciba en segundo plano (ahorro de batería)") {
                    startActivity(Intent(android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")))
                }
            },
            if (org.mobilemesh.sdk.Silencio.activo(this)) {
                text("Recepción instantánea: activada (gasta más batería)")
            } else {
                button("Activar recepción instantánea (gasta más batería)") {
                    org.mobilemesh.sdk.Silencio.activar(this, true)
                    status.text = "Recepción instantánea activada: los mensajes llegarán aunque la pantalla esté apagada"
                }
            },
            button("Probar ayudante: saludar") {
                // Funciones de servidor: llama a «saludar» del contacto «ayudante».
                thread {
                    val ruta = listOfNotNull(myAddr(), m.relayInfo()).joinToString("|")
                    val id = m.callHelper("ayudante", "saludar", "{\"desde\":\"${Build.MODEL}\"}", ruta)
                    val t0 = System.currentTimeMillis()
                    var r: String? = null
                    while (r == null && System.currentTimeMillis() - t0 < 30_000) {
                        Thread.sleep(200)
                        r = m.helperReply("ayudante", id)
                    }
                    val texto = r?.let { "Respuesta en ${System.currentTimeMillis() - t0} ms:\n$it" } ?: "El ayudante no respondió"
                    runOnUiThread { android.app.AlertDialog.Builder(this).setTitle("Ayudante").setMessage(texto).setPositiveButton("Cerrar", null).show() }
                }
            },
            button("Usar este relé") {
                val r = relayField.text.toString()
                work("Guardando…") {
                    m.setRelay(r.ifBlank { null })
                    if (r.isBlank()) "Ya no se usa relé (al reiniciar la app)" else "Relé guardado: se usará al reiniciar la app. Vuelve a compartir tu invitación."
                }
            },
            title("Añadir contacto"),
            button("Escanear invitación") { scan { invite.setText(it) } },
            invite,
            name,
            button("Añadir contacto") {
                val (i, n) = invite.text.toString() to name.text.toString()
                work("Añadiendo…") {
                    val parsed = parseInvite(i)
                    val own = m.addContact(n, parsed.key, parsed.addr)
                    ui.post { loadContacts(); invite.setText(""); name.setText(""); dataSig = "" }
                    if (own) "«$n» es otro dispositivo de tu cuenta: se sincronizará con él" else "Contacto «$n» añadido"
                }
            },
            title("Mensaje"),
            contacts,
            message,
            button("Enviar") { send(message.text.toString()) },
            button("Enviar foto o archivo") {
                val pick = Intent(Intent.ACTION_GET_CONTENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE)
                startActivityForResult(Intent.createChooser(pick, "Elige un archivo"), PICK_FILE)
            },
            button("Reintentar pendientes") { send("") },
            status,
            title("Datos sincronizados"),
            text("Se guardan al momento en este teléfono y se sincronizan con los otros dispositivos de tu cuenta."),
            Switch(this).apply {
                text = "Modo sin conexión"
                isChecked = m.isOffline()
                setOnCheckedChangeListener { _, on -> m.setOffline(on); dataSig = "" }
            },
            profileName,
            profileState,
            button("Guardar perfil") {
                val (n, e) = profileName.text.toString() to profileState.text.toString()
                work("Guardando…") {
                    if (n.isNotBlank()) m.put("perfil", "yo", "nombre", n, false)
                    if (e.isNotBlank()) m.put("perfil", "yo", "estado", e, false)
                    ui.post { profileName.setText(""); profileState.setText(""); dataSig = "" }
                    "Perfil guardado en este teléfono"
                }
            },
            balance,
            button("Guardar saldo") {
                val b = balance.text.toString()
                work("Guardando…") {
                    m.put("caja", "saldo", "valor", b, true)
                    ui.post { balance.setText(""); dataSig = "" }
                    "Saldo guardado en este teléfono"
                }
            },
            button("Sincronizar ahora") { work("Sincronizando…") { m.syncNow().also { ui.post { dataSig = "" } } } },
            LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }.also { dataBox = it; dataSig = "" },
            title("Copias de seguridad"),
            text("Tu copia va cifrada con la clave de tu cuenta: quien la guarda no puede leerla. Si pierdes el teléfono, recuperas la cuenta con tu copia de recuperación y después tus datos desde aquí."),
            copyName,
            button("Guardar mi copia en él") { copyOp("Añadiendo…") { m.addCopyHolder(it); "«$it» guardará tu copia. Pulsa «Hacer copia y enviarla»." } },
            button("Hacer copia y enviarla") { work("Cifrando y enviando la copia…") { m.backupNow().also { ui.post { copySig = "" } } } },
            button("Comprobar mis copias") { work("Comprobando…") { m.checkCopies().also { ui.post { copySig = "" } } } },
            button("Pedirle que borre mi copia") { copyOp("Pidiendo el borrado…") { m.removeCopyHolder(it) } },
            button("Recuperar mis datos desde él") { copyOp("Descargando y comprobando la copia…") { m.restoreCopy(it).also { ui.post { loadContacts(); dataSig = ""; inboxSig = "" } } } },
            button("Aceptar guardar copias suyas") { copyOp("Guardando…") { m.allowCopies(it, true); "Aceptas guardar copias cifradas de «$it» (no podrás leerlas)" } },
            button("Dejar de guardar sus copias") { copyOp("Borrando…") { m.allowCopies(it, false); "Ya no guardas copias de «$it»; la que tenías se borró" } },
            LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }.also { copyBox = it; copySig = "" },
            title("Permisos y listas compartidas"),
            who,
            button("Bloquear contacto") { permOp("Bloqueando…") { m.setBlocked(it, true); "«$it» bloqueado: no podrá escribirte ni conectarse" } },
            button("Desbloquear contacto") { permOp("Desbloqueando…") { m.setBlocked(it, false); "«$it» desbloqueado" } },
            button("Compartir mi lista: solo leer") { permOp("Compartiendo…") { m.share(m.listName(null), it, false); "«$it» puede ver tu lista, no cambiarla" } },
            button("Compartir mi lista: leer y cambiar") { permOp("Compartiendo…") { m.share(m.listName(null), it, true); "«$it» puede ver y cambiar tu lista" } },
            button("Dejar de compartir mi lista") { permOp("Quitando…") { m.share(m.listName(null), it, null); "Tu lista ya no se comparte con «$it»" } },
            button("Recibir su lista") { permOp("Aceptando…") { m.share(m.listName(it), it, true); "Recibirás la lista de «$it»" } },
            button("Sincronizar listas con él") { permOp("Sincronizando…") { m.syncWithContact(it) } },
            item,
            button("Añadir a mi lista") { addItem { m.listName(null) } },
            button("Añadir a su lista") {
                val n = who.text.toString().trim()
                if (n.isEmpty()) status.text = "Escribe el nombre del contacto" else addItem { m.listName(n) }
            },
            LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }.also { permBox = it; permSig = "" },
            title("Mensajes"),
            LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }.also { inbox = it; inboxSig = "" },
        )
        // El servicio escucha, reintenta y sincroniza aunque se cierre la app.
        MeshService.start(this)
    }

    private fun backupScreen() {
        val m = mesh!!
        status = TextView(this)
        val p1 = field("Contraseña (mínimo 10 caracteres)", secret = true)
        val p2 = field("Repite la contraseña", secret = true)
        page(
            title("Copia de recuperación"),
            text("Tu clave se cifra con esta contraseña. Guarda la copia fuera del teléfono (papel, otro equipo o un gestor de contraseñas) y no olvides la contraseña: nadie puede recuperarla por ti."),
            p1,
            p2,
            button("Crear copia") {
                val pass = p1.text.toString()
                if (pass != p2.text.toString()) return@button run { status.text = "Las contraseñas no coinciden" }
                work("Cifrando… (tarda unos segundos)") {
                    val code = m.backup(pass)
                    prefs.edit().putBoolean("backup", true).apply()
                    ui.post {
                        showQr("Tu copia de recuperación", "Guárdala fuera de este teléfono. Sin la contraseña no sirve, así que quien la fotografíe no puede usarla.", code, share = true)
                    }
                    null
                }
            },
            button("Volver") { mainScreen() },
            status,
        )
    }

    private fun showQr(t: String, explanation: String, content: String, share: Boolean) {
        page(
            title(t),
            text(explanation),
            qr(content),
            mono(content).apply { setTextIsSelectable(true) },
            if (!share) null else button("Compartir o guardar") {
                val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, content)
                startActivity(Intent.createChooser(send, "Guardar copia de recuperación"))
            },
            button("Volver") { mainScreen() },
        )
    }

    // ---------- Utilidades ----------

    private fun scan(onResult: (String) -> Unit) {
        onScan = onResult
        IntentIntegrator(this).setDesiredBarcodeFormats(IntentIntegrator.QR_CODE)
            .setPrompt("Apunta al código QR").setBeepEnabled(false).setOrientationLocked(false).initiateScan()
    }

    @Deprecated("API de Activity")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode == PICK_FILE) {
            if (resultCode == RESULT_OK) data?.data?.let { onPick?.invoke(it) }
            return
        }
        IntentIntegrator.parseActivityResult(requestCode, resultCode, data)?.contents?.let { onScan?.invoke(it) }
        super.onActivityResult(requestCode, resultCode, data)
    }

    /** Ejecuta una operación lenta fuera de la pantalla; si devuelve texto, lo muestra. */
    private fun work(label: String, op: () -> String?) {
        val target = status
        target.text = label
        thread {
            val result = try { op() } catch (e: Exception) { "Error: ${e.text()}" }
            if (result != null) ui.post { target.text = result }
        }
    }

    private fun resultado(r: uniffi.mobilemesh_core.SendResult) = if (r.inMailbox) {
        "${r.count} mensaje(s) dejado(s) en su buzón: lo recibirá cuando su teléfono despierte"
    } else {
        "${r.count} mensaje(s) entregado(s) y guardado(s) en el destino"
    }

    private fun Throwable.text() = (this as? MeshException.Fallo)?.msg ?: message ?: toString()

    // ponytail: refresco cada segundo; cambiar por un aviso desde Rust cuando llegue un mensaje.
    private fun refresh() {
        if (!visible) return
        ui.removeCallbacksAndMessages(null)
        val m = mesh
        val box = inbox
        val data = dataBox
        if (m != null && data != null) showData(m, data)
        val copies = copyBox
        if (m != null && copies != null) showCopies(m, copies)
        val perms = permBox
        if (m != null && perms != null) showPermissions(m, perms)
        if (m != null && box != null) {
            val msgs = m.messages().reversed()
            // Solo se redibuja si algo cambió (estado o progreso), para que las fotos no parpadeen.
            val sig = msgs.joinToString("|") { it.id + it.status + it.progress }
            if (sig != inboxSig) {
                inboxSig = sig
                box.removeAllViews()
                if (msgs.isEmpty()) box.addView(text("Todavía no hay mensajes"))
                msgs.forEach { box.addView(messageView(m, it)) }
            }
        }
        ui.postDelayed(::refresh, 1000)
    }

    /** Estado de los datos sincronizados: perfil, saldo (con sus conflictos) y qué falta por confirmar. */
    private fun showData(m: Mesh, box: LinearLayout) {
        val perfil = m.records("perfil").firstOrNull()
        val caja = m.records("caja").firstOrNull()
        val devices = m.myDevices()
        val pending = m.unsynced()
        val sig = listOf(perfil, caja, devices.map { it.name }, pending, m.isOffline()).toString()
        if (sig == dataSig) return
        dataSig = sig
        box.removeAllViews()
        box.addView(text(if (devices.isEmpty()) "Sin otros dispositivos: añade la invitación de tu otro teléfono." else "Sincroniza con: " + devices.joinToString { "${it.name} (${it.addr})" }))
        box.addView(text(when {
            m.isOffline() -> "Sin conexión: $pending cambio(s) esperando"
            pending == 0uL -> "Todo confirmado por tus dispositivos"
            else -> "$pending cambio(s) sin confirmar todavía"
        }))
        box.addView(mono("Perfil: " + (perfil?.fields?.joinToString { "${it.name}=${it.value}" } ?: "vacío")))
        box.addView(mono("Saldo: " + (caja?.fields?.firstOrNull()?.value ?: "sin datos")))
        caja?.conflicts?.forEach { c ->
            val current = caja.fields.firstOrNull { it.name == c.field }?.value
            val options = listOfNotNull(current) + c.values
            box.addView(warn("Conflicto: el saldo se cambió a la vez en dos dispositivos (${options.joinToString(" y ")}). Elige el correcto:"))
            options.distinct().forEach { v ->
                box.addView(button("Quedarme con $v") { work("Guardando…") { m.put("caja", "saldo", c.field, v, true); ui.post { dataSig = "" }; "Elegido: $v" } })
            }
        }
    }

    private fun showPermissions(m: Mesh, box: LinearLayout) {
        val name = permName?.text?.toString()?.trim().orEmpty()
        fun items(list: String) = m.records(list).mapNotNull { r -> r.fields.firstOrNull { it.name == "texto" }?.value }
        val mine = items(m.listName(null))
        val theirs = runCatching { if (name.isEmpty()) null else items(m.listName(name)) }.getOrNull()
        val devices = m.accountDevices()
        val shares = runCatching { if (name.isEmpty()) emptyList() else m.shares(name) }.getOrDefault(emptyList())
        val blocked = runCatching { name.isNotEmpty() && m.isBlocked(name) }.getOrDefault(false)
        val sig = listOf(name, mine, theirs, devices.map { it.key + it.revoked }, shares, blocked).toString()
        if (sig == permSig) return
        permSig = sig
        box.removeAllViews()
        box.addView(mono("Mi lista: " + mine.ifEmpty { listOf("vacía") }.joinToString()))
        if (theirs != null) box.addView(mono("Lista de $name: " + theirs.ifEmpty { listOf("vacía o sin recibir") }.joinToString()))
        if (name.isNotEmpty()) box.addView(text("Con «$name» compartes: " + shares.ifEmpty { listOf("nada") }.joinToString() + if (blocked) " — BLOQUEADO" else ""))
        box.addView(title("Dispositivos de mi cuenta"))
        devices.forEach { d ->
            val label = d.fingerprint + when { d.isMe -> " (este)"; d.revoked -> " (retirado)"; else -> "" }
            box.addView(mono(label))
            if (!d.isMe && !d.revoked && m.isPrimary()) {
                box.addView(button("Retirar ${d.fingerprint} (perdido o robado)") { work("Retirando y avisando…") { m.revokeDevice(d.key).also { ui.post { permSig = "" } } } })
            }
        }
    }

    private fun showCopies(m: Mesh, box: LinearLayout) {
        val holders = m.copyHolders()
        val held = m.heldCopies()
        val sig = holders.map { it.name + it.status }.toString() + held.map { it.owner + it.size }
        if (sig == copySig) return
        copySig = sig
        box.removeAllViews()
        box.addView(text(if (holders.isEmpty()) "Nadie guarda tu copia todavía." else "Guardan tu copia: " + holders.joinToString { "${it.name} (${it.status})" }))
        box.addView(text(if (held.isEmpty()) "No guardas copias de nadie." else "Guardas copias (cifradas) de: " + held.joinToString { "${it.owner} (${size(it.size)})" }))
    }

    private fun messageView(m: Mesh, msg: Message): View = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(0, 16, 0, 16)
        val state = when {
            msg.outgoing && msg.status == "pendiente" && msg.progress.toInt() > 0 -> "enviando ${msg.progress}%"
            msg.status == "en buzón" -> "en su buzón, esperando a que despierte"
            msg.status == "recibiendo" -> "recibiendo ${msg.progress}%"
            else -> msg.status
        }
        addView(mono("${if (msg.outgoing) "→" else "←"} ${msg.peer}  [$state]"))
        val name = msg.fileName
        if (name == null) return@apply addView(text(msg.body))
        addView(text("📎 $name (${size(msg.fileSize ?: 0u)})"))
        // El remitente tiene su copia desde el principio; el destino, solo cuando la comprobó entera.
        if (!msg.outgoing && msg.status != "guardado") return@apply
        if (!msg.fileAvailable) return@apply addView(text("(archivo no disponible en este teléfono: no va en las copias de seguridad)"))
        if (msg.mime?.startsWith("image/") == true) addView(thumbnail(m, msg))
        addView(button("Guardar en Descargas") { saveToDownloads(m, msg) })
    }

    private fun thumbnail(m: Mesh, msg: Message) = ImageView(this).apply {
        adjustViewBounds = true
        maxHeight = 700
        if (thumbs.containsKey(msg.id)) return@apply setImageBitmap(thumbs[msg.id])
        val view = this
        thread {
            // Se descifra a la caché, se reduce y se borra al momento: no queda la foto en claro en disco.
            val tmp = File(cacheDir, "vista-${msg.id}")
            val bmp = try {
                m.exportFile(msg.peerKey, msg.outgoing, msg.id, tmp.path)
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeFile(tmp.path, bounds)
                var sample = 1
                while (bounds.outWidth / sample > 1000 || bounds.outHeight / sample > 1000) sample *= 2
                BitmapFactory.decodeFile(tmp.path, BitmapFactory.Options().apply { inSampleSize = sample })
            } catch (e: Exception) {
                null
            } finally {
                tmp.delete()
            }
            ui.post { thumbs[msg.id] = bmp; view.setImageBitmap(bmp) }
        }
    }

    private fun saveToDownloads(m: Mesh, msg: Message) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            status.text = "Guardar en Descargas necesita Android 10 o superior"
            return
        }
        work("Descifrando y guardando…") {
            val tmp = File(cacheDir, "exporta-${msg.id}")
            try {
                m.exportFile(msg.peerKey, msg.outgoing, msg.id, tmp.path)
                val values = ContentValues().apply {
                    put(MediaStore.Downloads.DISPLAY_NAME, msg.fileName)
                    put(MediaStore.Downloads.MIME_TYPE, msg.mime)
                }
                val uri = contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)!!
                contentResolver.openOutputStream(uri)!!.use { out -> tmp.inputStream().use { it.copyTo(out) } }
                "Guardado en Descargas: ${msg.fileName}"
            } finally {
                tmp.delete()
            }
        }
    }

    private fun fileName(uri: Uri): String =
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        } ?: "archivo"

    private fun size(bytes: ULong): String = when {
        bytes < 1024u -> "$bytes B"
        bytes < 1024u * 1024u -> "${bytes / 1024u} KB"
        else -> "%.1f MB".format(bytes.toDouble() / 1024 / 1024)
    }

    private fun myAddr(): String {
        val ip = NetworkInterface.getNetworkInterfaces().toList()
            .flatMap { it.inetAddresses.toList() }
            .firstOrNull { it is Inet4Address && !it.isLoopbackAddress }?.hostAddress ?: "sin-red"
        return "$ip:${MeshHolder.puerto(this)}"
    }

    private fun qr(content: String) = ImageView(this).apply {
        setImageBitmap(BarcodeEncoder().encodeBitmap(content, BarcodeFormat.QR_CODE, 700, 700))
        adjustViewBounds = true
    }

    private fun page(vararg views: View?) {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(40, 40, 40, 40)
            views.filterNotNull().forEach { addView(it) }
        }
        setContentView(ScrollView(this).apply { fitsSystemWindows = true; addView(root) })
    }

    private fun field(hint: String, secret: Boolean = false) = EditText(this).apply {
        this.hint = hint
        isSingleLine = true
        if (secret) inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
    }

    private fun button(label: String, onClick: () -> Unit) = Button(this).apply {
        text = label
        setOnClickListener { onClick() }
    }

    private fun title(t: String) = TextView(this).apply {
        text = t
        textSize = 18f
        setTypeface(null, Typeface.BOLD)
        setPadding(0, 32, 0, 8)
    }

    private fun text(t: String) = TextView(this).apply { text = t; setPadding(0, 8, 0, 8) }

    private fun mono(t: String) = TextView(this).apply { text = t; typeface = Typeface.MONOSPACE }

    private fun warn(t: String) = TextView(this).apply {
        text = "⚠ $t"
        setTextColor(Color.rgb(160, 60, 0))
        setPadding(0, 12, 0, 12)
    }
}
