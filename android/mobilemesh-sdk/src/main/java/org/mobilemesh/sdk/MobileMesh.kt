package org.mobilemesh.sdk

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import java.io.File
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.NetworkInterface
import kotlin.concurrent.thread
import org.json.JSONArray
import org.json.JSONObject
import uniffi.mobilemesh_core.IdentityState
import uniffi.mobilemesh_core.Mesh
import uniffi.mobilemesh_core.MeshException
import uniffi.mobilemesh_core.PublicFilter
import uniffi.mobilemesh_core.QueryFilter
import uniffi.mobilemesh_core.SyncRecord
import uniffi.mobilemesh_core.identityState
import uniffi.mobilemesh_core.parseInvite

/**
 * MobileMesh error, with a message ready to show (English, then Spanish in parentheses).
 *
 * Error de MobileMesh, con un mensaje listo para mostrar.
 */
class MobileMeshError(message: String) : Exception(message)

/**
 * Account state on this phone.
 *
 * Estado de la cuenta en este teléfono.
 */
enum class AccountState {
    /**
     * First time: `start` will create the account.
     *
     * Primera vez: `start` creará la cuenta.
     */
    NO_ACCOUNT,

    /**
     * The account exists, but the phone is locked: it opens once unlocked.
     *
     * Hay cuenta, pero el teléfono está bloqueado: se abre al desbloquearlo.
     */
    LOCKED,

    /**
     * Ready to use.
     *
     * Lista para usar.
     */
    READY;

    companion object {
        @Deprecated("Use NO_ACCOUNT", ReplaceWith("AccountState.NO_ACCOUNT"))
        val SIN_CUENTA: AccountState get() = NO_ACCOUNT

        @Deprecated("Use LOCKED", ReplaceWith("AccountState.LOCKED"))
        val BLOQUEADO: AccountState get() = LOCKED

        @Deprecated("Use READY", ReplaceWith("AccountState.READY"))
        val LISTA: AccountState get() = READY
    }
}

@Deprecated("Use AccountState")
typealias EstadoCuenta = AccountState

/**
 * A document of a collection. `conflicts`: disputed values of strict collections.
 *
 * Un documento de una colección. `conflicts`: valores en disputa de las colecciones estrictas.
 */
data class Document(val id: String, val fields: Map<String, Any?>, val conflicts: Map<String, List<Any?>>) {
    @Deprecated("Use fields", ReplaceWith("fields"))
    val campos: Map<String, Any?> get() = fields

    @Deprecated("Use conflicts", ReplaceWith("conflicts"))
    val conflictos: Map<String, List<Any?>> get() = conflicts
}

@Deprecated("Use Document")
typealias Documento = Document

/**
 * Document of a public collection: `author` is the (verified) account that published it.
 *
 * Documento de una colección pública: `author` es la cuenta que lo publicó (verificada).
 */
data class PublicDocument(val id: String, val author: String, val fields: Map<String, Any?>) {
    @Deprecated("Use author", ReplaceWith("author"))
    val autor: String get() = author

    @Deprecated("Use fields", ReplaceWith("fields"))
    val campos: Map<String, Any?> get() = fields
}

@Deprecated("Use PublicDocument")
typealias DocumentoPublico = PublicDocument

/**
 * Result of a send: how many messages went out and whether they were left in the mailbox of a sleeping recipient.
 *
 * Resultado de un envío: cuántos mensajes y si quedaron en el buzón del destinatario dormido.
 */
data class Delivery(val count: Int, val inMailbox: Boolean) {
    @Deprecated("Use count", ReplaceWith("count"))
    val cuantos: Int get() = count

    @Deprecated("Use inMailbox", ReplaceWith("inMailbox"))
    val enBuzon: Boolean get() = inMailbox
}

@Deprecated("Use Delivery")
typealias Envio = Delivery

/**
 * A chat message (`sent`: true if outgoing).
 *
 * Un mensaje de chat (`sent`: true si lo envié yo).
 */
data class Message(val id: String, val contact: String, val sent: Boolean, val text: String, val status: String, val file: String?) {
    @Deprecated("Use contact", ReplaceWith("contact"))
    val contacto: String get() = contact

    @Deprecated("Use sent", ReplaceWith("sent"))
    val enviado: Boolean get() = sent

    @Deprecated("Use text", ReplaceWith("text"))
    val texto: String get() = text

    @Deprecated("Use status", ReplaceWith("status"))
    val estado: String get() = status

    @Deprecated("Use file", ReplaceWith("file"))
    val archivo: String? get() = file
}

@Deprecated("Use Message")
typealias Mensaje = Message

/**
 * MobileMesh for your app: database, accounts, messages, files, permissions and sync, with no
 * servers. Data lives encrypted on the phone and travels end-to-end encrypted.
 *
 * ```
 * val mesh = MobileMesh.start(context)
 * val orders = mesh.collection("orders")
 * orders.save("o1", mapOf("product" to "bread", "quantity" to 2))
 * orders.subscribe { docs -> show(docs) }
 * ```
 *
 * Network operations (`sendMessage`, `sendFile`, `syncWith`) run in the background and report with
 * `onDone` on the main thread. Everything else is immediate.
 *
 * MobileMesh para tu app: base de datos, cuentas, mensajes, archivos, permisos y sincronización,
 * sin servidores. Los datos viven cifrados en el teléfono y viajan cifrados de extremo a extremo.
 * Las operaciones de red trabajan en segundo plano y avisan con `onDone` en el hilo principal.
 */
class MobileMesh private constructor(private val context: Context, val core: Mesh) {

    // Cada app tiene su espacio en la guía de nombres: «juan» de esta app no choca con el de otra.
    init {
        wrap { core.setApp(context.packageName) }
    }

    @Deprecated("Use core", ReplaceWith("core"))
    val nucleo: Mesh get() = core

    companion object {
        private val ui = Handler(Looper.getMainLooper())

        /**
         * Leaves the ongoing notification without a status-bar icon (it sits at the very bottom of
         * the shade). Meant for apps that do not use messaging and are bothered by the permanent
         * notification.
         *
         * Call it BEFORE [start]: the channel is chosen when the service starts.
         *
         * Careful if the app does receive messages: some systems (TECNO HiOS, MIUI...) freeze apps
         * whose ongoing notification is not visible, and then messages stop arriving. That is why
         * it is off by default.
         *
         * Deja la notificación fija sin icono en la barra de estado. Pensado para apps que no usan
         * mensajería. Llámalo ANTES de [start]. Cuidado si la app sí recibe mensajes: algunos
         * sistemas congelan las apps cuya notificación fija no se ve; por eso está apagado por defecto.
         */
        fun discreetNotice(context: Context, enabled: Boolean) = AvisoFijo.discreto(context.applicationContext, enabled)

        @Deprecated("Use discreetNotice()", ReplaceWith("discreetNotice(context, activo)"))
        fun avisoDiscreto(context: Context, activo: Boolean) = discreetNotice(context, activo)

        /**
         * How much MobileMesh works while the app is closed. [BackgroundMode.FULL] by default, which
         * is how it has always behaved.
         *
         * - [BackgroundMode.FULL]: instant messages. Requires permissions that Google Play restricts
         *   and that must be justified.
         * - [BackgroundMode.ALARM]: notices every few minutes, with no ongoing notification and no
         *   restricted permissions.
         * - [BackgroundMode.NONE]: nothing in the background.
         *
         * Call it BEFORE [start].
         *
         * Cuánto trabaja MobileMesh con la app cerrada. [BackgroundMode.FULL] por defecto.
         * Llámalo ANTES de [start].
         */
        fun backgroundMode(context: Context, mode: BackgroundMode) = ServicioFondo.modo(context.applicationContext, mode)

        @Deprecated("Use backgroundMode()", ReplaceWith("backgroundMode(context, modo)"))
        fun modoFondo(context: Context, modo: BackgroundMode) = backgroundMode(context, modo)

        /**
         * Account state on this phone.
         *
         * Estado de la cuenta en este teléfono.
         */
        fun state(context: Context): AccountState {
            val guard = KeystoreGuard(context.applicationContext)
            return when (identityState(MeshHolder.dir(context), guard)) {
                IdentityState.NOTHING, IdentityState.PENDING -> AccountState.NO_ACCOUNT
                IdentityState.READY -> if (MeshHolder.open(context) != null) AccountState.READY else AccountState.LOCKED
            }
        }

        @Deprecated("Use state()", ReplaceWith("state(context)"))
        fun estado(context: Context): AccountState = state(context)

        /**
         * Opens the user's account (creating it the first time) and leaves MobileMesh receiving in
         * the background. `port`: different for each app on the same phone.
         * Fails with [MobileMeshError] if the phone is locked.
         *
         * Abre la cuenta del usuario (la crea la primera vez) y deja MobileMesh recibiendo en
         * segundo plano. `port`: distinto para cada app del mismo teléfono.
         * Falla con [MobileMeshError] si el teléfono está bloqueado.
         */
        fun start(context: Context, port: Int = DEFAULT_PORT): MobileMesh {
            val app = context.applicationContext
            MeshHolder.guardarPuerto(app, port)
            val m = MeshHolder.open(app) ?: run {
                if (state(app) == AccountState.LOCKED) throw MobileMeshError("The phone is locked: unlock it to open the account (El teléfono está bloqueado: desbloquéalo para abrir la cuenta)")
                wrap { Mesh.create(MeshHolder.dir(app), KeystoreGuard(app)) }.also { MeshHolder.mesh = it }
            }
            MeshService.start(app)
            CopiaAutomatica.siToca(app)
            return MobileMesh(app, m)
        }

        @Deprecated("Use start()", ReplaceWith("start(context, puerto)"))
        fun iniciar(context: Context, puerto: Int = DEFAULT_PORT): MobileMesh = start(context, puerto)

        /**
         * Recovers an account on a new phone with its recovery backup.
         *
         * Recupera una cuenta en un teléfono nuevo con su copia de recuperación.
         */
        fun recover(context: Context, code: String, password: String, port: Int = DEFAULT_PORT): MobileMesh {
            val app = context.applicationContext
            MeshHolder.guardarPuerto(app, port)
            val m = wrap { Mesh.restore(MeshHolder.dir(app), KeystoreGuard(app), code, password) }
            MeshHolder.mesh = m
            MeshService.start(app)
            return MobileMesh(app, m)
        }

        @Deprecated("Use recover()", ReplaceWith("recover(context, codigo, contrasena, puerto)"))
        fun recuperar(context: Context, codigo: String, contrasena: String, puerto: Int = DEFAULT_PORT): MobileMesh =
            recover(context, codigo, contrasena, puerto)

        /**
         * Lost phone: recovers the account with username and password from the guardian and brings
         * back the data from its backup (if there was one). Returns the account and a summary of
         * what was recovered.
         *
         * Teléfono perdido: recupera la cuenta con usuario y contraseña desde el guardián y trae los
         * datos de su copia (si la había). Devuelve la cuenta y un resumen de lo recuperado.
         */
        fun recoverWithGuardian(context: Context, username: String, password: String, port: Int = DEFAULT_PORT): Pair<MobileMesh, String> {
            val app = context.applicationContext
            MeshHolder.guardarPuerto(app, port)
            val m = wrap { Mesh.recoverFromGuardian(MeshHolder.dir(app), KeystoreGuard(app), username, password) }
            MeshHolder.mesh = m
            MeshService.start(app)
            val resumen = runCatching { m.restoreCopy(m.useGuardian()) }.getOrElse { "Cuenta recuperada; no había copia de datos" }
            return MobileMesh(app, m) to resumen
        }

        @Deprecated("Use recoverWithGuardian()", ReplaceWith("recoverWithGuardian(context, usuario, contrasena, puerto)"))
        fun recuperarConGuardian(context: Context, usuario: String, contrasena: String, puerto: Int = DEFAULT_PORT): Pair<MobileMesh, String> =
            recoverWithGuardian(context, usuario, contrasena, puerto)

        /**
         * Forgotten password: recovers the account with the paper code (`paperCode`) and brings back
         * the data from its backup in the guardian (if there was one). Returns the account and a summary.
         *
         * Contraseña olvidada: recupera la cuenta con el código en papel (`paperCode`) y trae los
         * datos de su copia en el guardián (si la había). Devuelve la cuenta y un resumen.
         */
        fun recoverWithPaper(context: Context, code: String, port: Int = DEFAULT_PORT): Pair<MobileMesh, String> {
            val app = context.applicationContext
            MeshHolder.guardarPuerto(app, port)
            val m = wrap { Mesh.restorePaper(MeshHolder.dir(app), KeystoreGuard(app), code) }
            MeshHolder.mesh = m
            MeshService.start(app)
            val resumen = runCatching { m.restoreCopy(m.useGuardian()) }.getOrElse { "Cuenta recuperada; no había copia de datos" }
            return MobileMesh(app, m) to resumen
        }

        @Deprecated("Use recoverWithPaper()", ReplaceWith("recoverWithPaper(context, codigo, puerto)"))
        fun recuperarConPapel(context: Context, codigo: String, puerto: Int = DEFAULT_PORT): Pair<MobileMesh, String> =
            recoverWithPaper(context, codigo, puerto)

        internal fun <T> wrap(op: () -> T): T = try {
            op()
        } catch (e: MeshException.Fallo) {
            throw MobileMeshError(e.msg)
        }

        internal fun <T> inBackground(op: () -> T, onDone: (Result<T>) -> Unit) {
            thread {
                val r = runCatching { wrap(op) }
                ui.post { onDone(r) }
            }
        }
    }

    /**
     * Public identifier of the account.
     *
     * Identificador público de la cuenta.
     */
    val myId: String get() = core.id()

    @Deprecated("Use myId", ReplaceWith("myId"))
    val miId: String get() = myId

    internal val contexto: Context get() = context

    /**
     * Opens (or creates) a collection of documents. `strict`: concurrent changes are not merged automatically.
     *
     * Abre (o crea) una colección de documentos. `strict`: los cambios simultáneos no se mezclan solos.
     */
    fun collection(name: String, strict: Boolean = false) = MeshCollection(this, name, strict)

    @Deprecated("Use collection()", ReplaceWith("collection(nombre, estricta)"))
    fun coleccion(nombre: String, estricta: Boolean = false) = collection(nombre, estricta)

    /**
     * Public collection: what the app deliberately publishes for all its users to see (ranking,
     * catalog, board). It lives in the public piece, not on the phone; everything else stays
     * private. Each document carries its author and nobody can change someone else's.
     *
     * Colección pública: lo que la app publica a propósito para que lo vean todos sus usuarios
     * (ranking, catálogo, tablón). Vive en la pieza pública, no en el móvil; todo lo demás sigue
     * privado. Cada documento lleva su autor y nadie puede cambiar lo de otro.
     */
    fun publicCollection(name: String) = PublicCollection(this, name)

    @Deprecated("Use publicCollection()", ReplaceWith("publicCollection(nombre)"))
    fun coleccionPublica(nombre: String) = publicCollection(nombre)

    /**
     * Uses your own public piece (its invitation) instead of the universal one; `null` goes back to the universal one.
     *
     * Usa tu propia pieza pública (su invitación) en vez de la universal; `null` vuelve a la universal.
     */
    fun usePublicServer(invitation: String?) = wrap { core.usePublicServer(invitation) }

    @Deprecated("Use usePublicServer()", ReplaceWith("usePublicServer(invitacion)"))
    fun usarPiezaPublica(invitacion: String?) = usePublicServer(invitacion)

    // ---------- Contactos ----------

    /**
     * Invitation (text for a QR) with which another person adds you. It only carries public data.
     *
     * Invitación (texto para un QR) con la que otro te añade. Solo lleva datos públicos.
     */
    fun invitation(): String {
        val ruta = rutaDirecta() ?: throw MobileMeshError("No network connection: connect to wifi or mobile data to create your invitation (No hay conexión a ninguna red: conéctate a una wifi o a datos para crear tu invitación)")
        return wrap { core.invite(ruta) }
    }

    @Deprecated("Use invitation()", ReplaceWith("invitation()"))
    fun invitacion(): String = invitation()

    /**
     * Adds a contact with their invitation. Returns `true` if it was another device of your account.
     *
     * Añade un contacto con su invitación. Devuelve `true` si era otro dispositivo de tu cuenta.
     */
    fun addContact(name: String, invitation: String): Boolean = wrap {
        val i = parseInvite(invitation)
        core.addContact(name, i.key, i.addr)
    }

    @Deprecated("Use addContact()", ReplaceWith("addContact(nombre, invitacion)"))
    fun anadirContacto(nombre: String, invitacion: String): Boolean = addContact(nombre, invitacion)

    /**
     * Names of your contacts.
     *
     * Nombres de tus contactos.
     */
    fun contacts(): List<String> = wrap { core.contacts().map { it.name } }

    @Deprecated("Use contacts()", ReplaceWith("contacts()"))
    fun contactos(): List<String> = contacts()

    // ---------- Usuarios por nombre (guía de nombres) ----------

    /**
     * Registers (or updates) my username in the name guide so others can find me.
     *
     * Registra (o actualiza) mi nombre de usuario en la guía para que otros me encuentren.
     */
    fun registerName(username: String) = wrap { core.registerName(username, invitation()) }

    @Deprecated("Use registerName()", ReplaceWith("registerName(usuario)"))
    fun registrarNombre(usuario: String) = registerName(usuario)

    /**
     * Does this user exist?
     *
     * ¿Existe este usuario?
     */
    fun userExists(username: String): Boolean = wrap { core.findName(username) != null }

    @Deprecated("Use userExists()", ReplaceWith("userExists(usuario)"))
    fun existeUsuario(usuario: String): Boolean = userExists(usuario)

    /**
     * Adds a user by name and notifies them: their app will add me automatically (see `acceptRequests`).
     *
     * Añade a un usuario por su nombre y le avisa: su app me añadirá sola (ver `acceptRequests`).
     */
    fun addByName(username: String) = wrap { core.addByName(username) }

    @Deprecated("Use addByName()", ReplaceWith("addByName(usuario)"))
    fun anadirPorNombre(usuario: String) = addByName(usuario)

    /**
     * Adds as contacts those who want to talk to me. Returns their names.
     *
     * Añade como contactos a quienes quieren hablar conmigo. Devuelve sus nombres.
     */
    fun acceptRequests(): List<String> = wrap { core.acceptRequests() }

    @Deprecated("Use acceptRequests()", ReplaceWith("acceptRequests()"))
    fun aceptarSolicitudes(): List<String> = acceptRequests()

    /**
     * Uses another name guide (its invitation), or the universal one with `null`.
     *
     * Usa otra guía de nombres (su invitación), o la universal con `null`.
     */
    fun useNameGuide(invitation: String?) = wrap { core.useNameGuide(invitation) }

    @Deprecated("Use useNameGuide()", ReplaceWith("useNameGuide(invitacion)"))
    fun usarGuiaDeNombres(invitacion: String?) = useNameGuide(invitacion)

    /**
     * Blocks (or unblocks, with `blocked = false`) a contact.
     *
     * Bloquea (o desbloquea, con `blocked = false`) a un contacto.
     */
    fun block(contact: String, blocked: Boolean = true) = wrap { core.setBlocked(contact, blocked) }

    @Deprecated("Use block()", ReplaceWith("block(contacto, bloqueado)"))
    fun bloquear(contacto: String, bloqueado: Boolean = true) = block(contacto, bloqueado)

    // ---------- Mensajes y archivos ----------

    /**
     * Sends a message. If the destination is asleep and has a mailbox on a relay, it stays there and
     * is picked up on waking (`inMailbox = true`); otherwise it stays pending and is retried automatically.
     *
     * Envía un mensaje. Si el destino está dormido y tiene buzón en un relé, queda allí y lo recoge al
     * despertar (`inMailbox = true`); si no hay forma, queda pendiente y se reintenta solo.
     */
    fun sendMessage(contact: String, text: String, onDone: (Result<Delivery>) -> Unit = {}) =
        inBackground({ core.send(contact, text).let { Delivery(it.count.toInt(), it.inMailbox) } }, onDone).also { CopiaAutomatica.cambio(context) }

    @Deprecated("Use sendMessage()", ReplaceWith("sendMessage(contacto, texto, alTerminar)"))
    fun enviarMensaje(contacto: String, texto: String, alTerminar: (Result<Delivery>) -> Unit = {}) =
        sendMessage(contacto, texto, alTerminar)

    /**
     * All messages.
     *
     * Todos los mensajes.
     */
    fun messages(): List<Message> = wrap {
        core.messages().map { Message(it.id, it.peer, it.outgoing, it.body, it.status, it.fileName) }
    }

    @Deprecated("Use messages()", ReplaceWith("messages()"))
    fun mensajes(): List<Message> = messages()

    /**
     * Sends a file chosen by the user (photo, video, document). An encrypted copy is kept.
     *
     * Envía un archivo elegido por el usuario (foto, vídeo, documento). Se guarda una copia cifrada.
     */
    fun sendFile(contact: String, file: Uri, name: String, onDone: (Result<Delivery>) -> Unit = {}) =
        inBackground({
            val tmp = File(context.cacheDir, "salida-${System.nanoTime()}")
            try {
                context.contentResolver.openInputStream(file)!!.use { input -> tmp.outputStream().use { input.copyTo(it) } }
                val tipo = context.contentResolver.getType(file) ?: "application/octet-stream"
                core.sendFile(contact, tmp.path, name, tipo).let { Delivery(it.count.toInt(), it.inMailbox) }.also { CopiaAutomatica.cambio(context) }
            } finally {
                tmp.delete()
            }
        }, onDone)

    @Deprecated("Use sendFile()", ReplaceWith("sendFile(contacto, archivo, nombre, alTerminar)"))
    fun enviarArchivo(contacto: String, archivo: Uri, nombre: String, alTerminar: (Result<Delivery>) -> Unit = {}) =
        sendFile(contacto, archivo, nombre, alTerminar)

    /**
     * Notifies you (on the main thread) each time something new arrives. Returns how to stop listening.
     *
     * Te avisa (en el hilo principal) cada vez que llega algo nuevo. Devuelve cómo dejar de escuchar.
     */
    fun onReceive(listener: (count: Int) -> Unit): () -> Unit {
        MeshHolder.oyentes.add(listener)
        return { MeshHolder.oyentes.remove(listener) }
    }

    @Deprecated("Use onReceive()", ReplaceWith("onReceive(oyente)"))
    fun alRecibir(oyente: (cuantos: Int) -> Unit): () -> Unit = onReceive(oyente)

    // ---------- Funciones de servidor (ayudantes) ----------

    /**
     * Calls the function `function` of a helper (added as a contact with its invitation) and returns
     * its result (text or JSON). The helper knows for certain which account is calling.
     * ```
     * mesh.call("shop", "charge", mapOf("order" to "o1", "total" to 12.5)) { r -> ... }
     * ```
     *
     * Llama a la función `function` de un ayudante (añadido como contacto con su invitación) y
     * devuelve su resultado (texto o JSON). El ayudante sabe con certeza qué cuenta le llama.
     */
    fun call(helper: String, function: String, data: Map<String, Any?> = emptyMap(), onDone: (Result<String>) -> Unit) =
        inBackground({
            val ruta = listOfNotNull(rutaDirecta(), core.relayInfo()).joinToString("|")
            val id = core.callHelper(helper, function, JSONObject(data).toString(), ruta)
            val hasta = System.currentTimeMillis() + 60_000
            var respuesta: String? = null
            while (respuesta == null) {
                if (System.currentTimeMillis() > hasta) throw MobileMeshError("Helper «$helper» did not respond within 60 s (El ayudante «$helper» no respondió en 60 s)")
                Thread.sleep(250)
                respuesta = core.helperReply(helper, id)
            }
            respuesta
        }, onDone)

    @Deprecated("Use call()", ReplaceWith("call(ayudante, funcion, datos, alTerminar)"))
    fun llamar(ayudante: String, funcion: String, datos: Map<String, Any?> = emptyMap(), alTerminar: (Result<String>) -> Unit) =
        call(ayudante, funcion, datos, alTerminar)

    // ---------- Cuenta, red y diagnóstico ----------

    /**
     * Recovery backup encrypted with `password` (at least 10 characters). Main device only.
     *
     * Copia de recuperación cifrada con `password` (mínimo 10 caracteres). Solo el principal.
     */
    fun recoveryBackup(password: String): String = wrap { core.backup(password) }

    @Deprecated("Use recoveryBackup()", ReplaceWith("recoveryBackup(contrasena)"))
    fun copiaDeRecuperacion(contrasena: String): String = recoveryBackup(contrasena)

    /**
     * Paper code (56 letters, no password): recovers the account even if the password is forgotten
     * (`MobileMesh.recoverWithPaper`). Whoever has it gets into the account: show it so it is written
     * down on paper, never store it on the phone. Main device only.
     *
     * Código en papel (56 letras, sin contraseña): recupera la cuenta aunque se olvide la contraseña
     * (`MobileMesh.recoverWithPaper`). Quien lo tenga entra en la cuenta: enséñalo para que se apunte
     * en papel, nunca lo guardes en el teléfono. Solo el dispositivo principal.
     */
    fun paperCode(): String = wrap { core.paperCode() }

    @Deprecated("Use paperCode()", ReplaceWith("paperCode()"))
    fun codigoEnPapel(): String = paperCode()

    /**
     * Protection against losing the phone: stores the account code in the guardian (encrypted with
     * `password`, at least 12 characters) and an encrypted copy of the data. The guardian cannot read
     * anything. Repeat it from time to time to keep the data copy up to date.
     *
     * Protección contra pérdida del teléfono: guarda en el guardián el código de la cuenta (cifrado
     * con `password`, mínimo 12 caracteres) y una copia cifrada de los datos. El guardián no puede
     * leer nada. Repetirla de vez en cuando para que la copia de datos esté al día.
     */
    fun protectWithGuardian(username: String, password: String): String = wrap {
        core.saveAccountInGuardian(username, password)
        core.useGuardian()
        core.backupNow().also { context.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE).edit().putLong("ultima_copia", System.currentTimeMillis()).apply() }
    }

    @Deprecated("Use protectWithGuardian()", ReplaceWith("protectWithGuardian(usuario, contrasena)"))
    fun protegerConGuardian(usuario: String, contrasena: String): String = protectWithGuardian(usuario, contrasena)

    /**
     * Uses your own relay "host:port" to reach other networks (applied when the app restarts).
     *
     * Usa tu propio relé «host:puerto» para llegar a otras redes (se aplica al reiniciar la app).
     */
    fun useRelay(address: String?) = wrap { core.setRelay(address) }

    @Deprecated("Use useRelay()", ReplaceWith("useRelay(direccion)"))
    fun usarRele(direccion: String?) = useRelay(direccion)

    /**
     * Turns offline mode on or off.
     *
     * Activa o desactiva el modo sin conexión.
     */
    fun offlineMode(enabled: Boolean) = core.setOffline(enabled)

    @Deprecated("Use offlineMode()", ReplaceWith("offlineMode(activo)"))
    fun modoSinConexion(activo: Boolean) = offlineMode(activo)

    /**
     * Help other users: this phone acts as a relay (supernode) only on wifi and with battery
     * (15% or more, or charging). On by default; `false` turns it off (e.g. if the user does not
     * want it). Worth stating in the app's privacy policy. It cannot read anything that passes
     * through it. Takes effect in under a minute.
     *
     * Ayudar a otros usuarios: este teléfono hace de relé (supernodo) solo en wifi y con batería
     * (15 % o más, o cargando). Encendido por defecto; `false` lo apaga. Conviene decirlo en la
     * política de privacidad de la app. No puede leer nada de lo que pasa por él.
     */
    fun actAsSupernode(enabled: Boolean) = Supernodo.permitir(context, enabled)

    @Deprecated("Use actAsSupernode()", ReplaceWith("actAsSupernode(activo)"))
    fun hacerDeSupernodo(activo: Boolean) = actAsSupernode(activo)

    /**
     * Is it acting as a supernode right now?
     *
     * ¿Está haciendo de supernodo ahora mismo?
     */
    fun isSupernode(): Boolean = core.isSupernode()

    @Deprecated("Use isSupernode()", ReplaceWith("isSupernode()"))
    fun esSupernodo(): Boolean = isSupernode()

    /**
     * How often (in minutes) it checks the mailbox while the phone sleeps (1-60; 3 by default).
     * Fewer minutes: earlier notices, a bit more battery and data. Android spaces it out on its own in deep idle.
     *
     * Cada cuántos minutos mira el buzón con el teléfono dormido (1–60; 3 por defecto). Menos minutos:
     * avisos antes, algo más de batería y datos. Android lo espacia por su cuenta en reposo profundo.
     */
    fun checkMailboxEvery(minutes: Int) = Despertador.cambiarMinutos(context, minutes)

    @Deprecated("Use checkMailboxEvery()", ReplaceWith("checkMailboxEvery(minutos)"))
    fun mirarBuzonCada(minutos: Int) = checkMailboxEvery(minutos)

    /**
     * Instant notices without Google, also with the screen off on phones that freeze apps (TECNO,
     * ZTE, Xiaomi...): it keeps a silent sound playing so they do not freeze it. On by default;
     * with battery under 15% (not charging) it pauses and notices arrive through the alarm. `false`
     * turns it off (notices every few minutes, a bit less battery use).
     *
     * Avisos al instante sin Google, también con la pantalla apagada en teléfonos que congelan las
     * apps (TECNO, ZTE, Xiaomi…): mantiene un sonido en silencio para que no la congelen. Encendida
     * por defecto; con batería por debajo del 15 % (sin cargar) se pausa y los avisos llegan por el
     * despertador. `false` la apaga.
     */
    fun instantReception(enabled: Boolean) = Silencio.activar(context, enabled)

    @Deprecated("Use instantReception()", ReplaceWith("instantReception(activa)"))
    fun recepcionInstantanea(activa: Boolean) = instantReception(activa)

    /**
     * Summary of the state for debugging (no private data).
     *
     * Resumen del estado para depurar (sin datos privados).
     */
    fun diagnostics(): String = wrap { core.diagnostics() }

    @Deprecated("Use diagnostics()", ReplaceWith("diagnostics()"))
    fun diagnostico(): String = diagnostics()

    /**
     * Mis direcciones directas: la IPv4 de la red en que estoy, la pública si el router abrió el
     * puerto (UPnP) y la IPv6 pública. Con las dos últimas se llega directo desde otras redes.
     */
    private fun rutaDirecta(): String? {
        val p = MeshHolder.puerto(context)
        val addrs = NetworkInterface.getNetworkInterfaces().toList().filter { it.isUp && !it.isLoopback }.flatMap { it.inetAddresses.toList() }
        val v4 = addrs.firstOrNull { it is Inet4Address }?.hostAddress?.let { "$it:$p" }
        // Pública: 2000::/3 (no la local de enlace ni la de sitio).
        val v6 = addrs.firstOrNull { it is Inet6Address && (it.address[0].toInt() and 0xe0) == 0x20 }
            ?.hostAddress?.substringBefore('%')?.let { "[$it]:$p" }
        // La pública (puerto abierto por UPnP en el router de casa): llega desde datos móviles.
        return listOfNotNull(v4, core.publicRoute(), v6).joinToString("|").ifEmpty { null }
    }
}

/**
 * A collection of documents. Each field is stored encrypted on the phone and synced with your other
 * devices and with whoever you share it with. `strict`: changes made at the same time in two places
 * are not merged automatically (for money, inventory...); they stay in `Document.conflicts` until chosen.
 *
 * Una colección de documentos. Cada campo se guarda cifrado en el teléfono y se sincroniza con tus
 * otros dispositivos y con quien la compartas. `strict`: los cambios hechos a la vez en dos sitios
 * no se mezclan solos (para dinero, inventario…), quedan en `Document.conflicts` hasta elegir.
 */
class MeshCollection internal constructor(private val mesh: MobileMesh, val name: String, private val strict: Boolean) {

    @Deprecated("Use name", ReplaceWith("name"))
    val nombre: String get() = name

    /**
     * Saves fields of a document (text, numbers, booleans...). A `null` value deletes that field.
     *
     * Guarda campos de un documento (texto, números, booleanos…). Un valor `null` borra ese campo.
     */
    fun save(id: String, fields: Map<String, Any?>) = MobileMesh.wrap {
        fields.forEach { (field, value) -> mesh.core.put(name, id, field, value?.let { JSONArray().put(it).toString() }, strict) }
        CopiaAutomatica.cambio(mesh.contexto)
    }

    @Deprecated("Use save()", ReplaceWith("save(id, campos)"))
    fun guardar(id: String, campos: Map<String, Any?>) = save(id, campos)

    /**
     * Reads a document by id (`null` if it does not exist).
     *
     * Lee un documento por su id (`null` si no existe).
     */
    fun read(id: String): Document? = all().firstOrNull { it.id == id }

    @Deprecated("Use read()", ReplaceWith("read(id)"))
    fun leer(id: String): Document? = read(id)

    /**
     * All documents of the collection.
     *
     * Todos los documentos de la colección.
     */
    fun all(): List<Document> = MobileMesh.wrap { mesh.core.records(name).map(::toDocument) }

    @Deprecated("Use all()", ReplaceWith("all()"))
    fun todos(): List<Document> = all()

    /**
     * Query: `orders.where("price", ">", 10).where("paid", "==", false).sortBy("price").limit(20).get()`.
     * Operators: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza` (case-insensitive), `existe`.
     *
     * Consulta: `orders.where("price", ">", 10).where("paid", "==", false).sortBy("price").limit(20).get()`.
     * Operadores: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza` (sin distinguir mayúsculas), `existe`.
     */
    fun where(field: String, operator: String, value: Any? = null) = Query(this).where(field, operator, value)

    @Deprecated("Use where()", ReplaceWith("where(campo, operador, valor)"))
    fun donde(campo: String, operador: String, valor: Any? = null) = where(campo, operador, valor)

    /**
     * Starts a query sorted by `field`.
     *
     * Empieza una consulta ordenada por `field`.
     */
    fun sortBy(field: String, descending: Boolean = false) = Query(this).sortBy(field, descending)

    @Deprecated("Use sortBy()", ReplaceWith("sortBy(campo, descendente)"))
    fun ordenarPor(campo: String, descendente: Boolean = false) = sortBy(campo, descendente)

    internal fun runQuery(q: Query): List<Document> = MobileMesh.wrap {
        val filters = q.filters.map { (field, op, v) -> QueryFilter(field, op, JSONArray().put(v ?: JSONObject.NULL).toString()) }
        mesh.core.query(name, filters, q.order, q.descending, q.max?.toUInt()).map(::toDocument)
    }

    private fun toDocument(r: SyncRecord) = Document(
        r.id,
        r.fields.associate { it.name to value(it.value) },
        r.conflicts.associate { c -> c.field to c.values.map(::value) },
    )

    /**
     * Deletes a document (on all devices once synced).
     *
     * Borra un documento (en todos los dispositivos al sincronizar).
     */
    fun delete(id: String) {
        read(id)?.let { doc -> save(id, doc.fields.mapValues { null }) }
    }

    @Deprecated("Use delete()", ReplaceWith("delete(id)"))
    fun borrar(id: String) = delete(id)

    /**
     * Calls `listener` with all the documents now and every time they change (by you, by your other
     * devices or by whoever you share it with). Returns how to stop listening.
     *
     * Llama a `listener` con todos los documentos ahora y cada vez que cambien (por ti, por tus otros
     * dispositivos o por quien la compartas). Devuelve cómo dejar de escuchar.
     */
    // ponytail: comprueba cada segundo; cambiar por un aviso del núcleo si hace falta más inmediatez.
    fun subscribe(listener: (List<Document>) -> Unit): () -> Unit = subscribeTo(::all, listener)

    @Deprecated("Use subscribe()", ReplaceWith("subscribe(oyente)"))
    fun escuchar(oyente: (List<Document>) -> Unit): () -> Unit = subscribe(oyente)

    internal fun subscribeTo(source: () -> List<Document>, listener: (List<Document>) -> Unit): () -> Unit {
        val ui = Handler(Looper.getMainLooper())
        var ultimo: List<Document>? = null
        var activo = true
        fun mirar() {
            if (!activo) return
            val ahora = runCatching { source() }.getOrNull()
            if (ahora != null && ahora != ultimo) {
                ultimo = ahora
                listener(ahora)
            }
            ui.postDelayed(::mirar, 1000)
        }
        mirar()
        return { activo = false }
    }

    /**
     * Shares the collection with a contact. `canChange = false`: they can only read it.
     *
     * Comparte la colección con un contacto. `canChange = false`: solo puede leerla.
     */
    fun shareWith(contact: String, canChange: Boolean) = MobileMesh.wrap { mesh.core.share(name, contact, canChange) }

    @Deprecated("Use shareWith()", ReplaceWith("shareWith(contacto, puedeCambiar)"))
    fun compartirCon(contacto: String, puedeCambiar: Boolean) = shareWith(contacto, puedeCambiar)

    /**
     * Stops sharing the collection with a contact.
     *
     * Deja de compartir la colección con un contacto.
     */
    fun unshareWith(contact: String) = MobileMesh.wrap { mesh.core.share(name, contact, null) }

    @Deprecated("Use unshareWith()", ReplaceWith("unshareWith(contacto)"))
    fun dejarDeCompartirCon(contacto: String) = unshareWith(contacto)

    /**
     * Syncs now with a contact (it also happens automatically every few seconds).
     *
     * Sincroniza ahora con un contacto (también se hace solo cada pocos segundos).
     */
    fun syncWith(contact: String, onDone: (Result<String>) -> Unit = {}) =
        MobileMesh.inBackground({ mesh.core.syncWithContact(contact) }, onDone)

    @Deprecated("Use syncWith()", ReplaceWith("syncWith(contacto, alTerminar)"))
    fun sincronizarCon(contacto: String, alTerminar: (Result<String>) -> Unit = {}) = syncWith(contacto, alTerminar)

    private fun value(guardado: String): Any? = runCatching { JSONArray(guardado).get(0) }.getOrDefault(guardado)
}

@Deprecated("Use MeshCollection")
typealias Coleccion = MeshCollection

/**
 * Query over a collection: chain it and finish with `get()` or `subscribe { }`.
 *
 * Consulta sobre una colección: se encadena y se termina con `get()` o `subscribe { }`.
 */
class Query internal constructor(private val collection: MeshCollection) {
    internal val filters = mutableListOf<Triple<String, String, Any?>>()
    internal var order: String? = null
    internal var descending = false
    internal var max: Int? = null

    /**
     * Adds a filter.
     *
     * Añade un filtro.
     */
    fun where(field: String, operator: String, value: Any? = null) = apply { filters += Triple(field, operator, value) }

    @Deprecated("Use where()", ReplaceWith("where(campo, operador, valor)"))
    fun donde(campo: String, operador: String, valor: Any? = null) = where(campo, operador, valor)

    /**
     * Sorts by `field`.
     *
     * Ordena por `field`.
     */
    fun sortBy(field: String, descending: Boolean = false) = apply { order = field; this.descending = descending }

    @Deprecated("Use sortBy()", ReplaceWith("sortBy(campo, descendente)"))
    fun ordenarPor(campo: String, descendente: Boolean = false) = sortBy(campo, descendente)

    /**
     * At most `n` documents.
     *
     * Como mucho `n` documentos.
     */
    fun limit(n: Int) = apply { max = n }

    @Deprecated("Use limit()", ReplaceWith("limit(n)"))
    fun limite(n: Int) = limit(n)

    /**
     * Runs the query and returns the documents.
     *
     * Ejecuta la consulta y devuelve los documentos.
     */
    fun get(): List<Document> = collection.runQuery(this)

    @Deprecated("Use get()", ReplaceWith("get()"))
    fun obtener(): List<Document> = get()

    /**
     * Like `MeshCollection.subscribe`, but only with the documents of the query.
     *
     * Como `MeshCollection.subscribe`, pero solo con los documentos de la consulta.
     */
    fun subscribe(listener: (List<Document>) -> Unit): () -> Unit = collection.subscribeTo(::get, listener)

    @Deprecated("Use subscribe()", ReplaceWith("subscribe(oyente)"))
    fun escuchar(oyente: (List<Document>) -> Unit): () -> Unit = subscribe(oyente)
}

@Deprecated("Use Query")
typealias Consulta = Query

/**
 * Public collection (see [MobileMesh.publicCollection]). Operations go over the network: they run
 * in the background and report with `onDone` on the main thread.
 *
 * Colección pública (ver [MobileMesh.publicCollection]). Las operaciones van por la red: trabajan en
 * segundo plano y avisan con `onDone` en el hilo principal.
 */
class PublicCollection internal constructor(internal val mesh: MobileMesh, val name: String) {

    @Deprecated("Use name", ReplaceWith("name"))
    val nombre: String get() = name

    /**
     * Publishes (or updates) document `id`, only if it is mine. At most 4 KB of fields.
     *
     * Publica (o actualiza) el documento `id`, solo si es mío. Como mucho 4 KB de campos.
     */
    fun publish(id: String, fields: Map<String, Any?>, onDone: (Result<Unit>) -> Unit = {}) =
        MobileMesh.inBackground({ mesh.core.publish(name, id, JSONObject(fields).toString()) }, onDone)

    @Deprecated("Use publish()", ReplaceWith("publish(id, campos, alTerminar)"))
    fun publicar(id: String, campos: Map<String, Any?>, alTerminar: (Result<Unit>) -> Unit = {}) = publish(id, campos, alTerminar)

    /**
     * Deletes a document I published.
     *
     * Borra un documento que publiqué.
     */
    fun delete(id: String, onDone: (Result<Unit>) -> Unit = {}) =
        MobileMesh.inBackground({ mesh.core.unpublish(name, id) }, onDone)

    @Deprecated("Use delete()", ReplaceWith("delete(id, alTerminar)"))
    fun borrar(id: String, alTerminar: (Result<Unit>) -> Unit = {}) = delete(id, alTerminar)

    /**
     * Queries what everyone published: `ranking.sortBy("points", true).limit(10).get { ... }`.
     *
     * Consulta lo publicado por todos: `ranking.sortBy("points", true).limit(10).get { ... }`.
     */
    fun where(field: String, operator: String, value: Any? = null) = PublicQuery(this).where(field, operator, value)

    @Deprecated("Use where()", ReplaceWith("where(campo, operador, valor)"))
    fun donde(campo: String, operador: String, valor: Any? = null) = where(campo, operador, valor)

    /**
     * Starts a query sorted by `field`.
     *
     * Empieza una consulta ordenada por `field`.
     */
    fun sortBy(field: String, descending: Boolean = false) = PublicQuery(this).sortBy(field, descending)

    @Deprecated("Use sortBy()", ReplaceWith("sortBy(campo, descendente)"))
    fun ordenarPor(campo: String, descendente: Boolean = false) = sortBy(campo, descendente)

    /**
     * Gets all the published documents (up to 50).
     *
     * Obtiene los documentos publicados (hasta 50).
     */
    fun get(onDone: (Result<List<PublicDocument>>) -> Unit) = PublicQuery(this).get(onDone)

    @Deprecated("Use get()", ReplaceWith("get(alTerminar)"))
    fun obtener(alTerminar: (Result<List<PublicDocument>>) -> Unit) = get(alTerminar)
}

@Deprecated("Use PublicCollection")
typealias ColeccionPublica = PublicCollection

/**
 * Query over a public collection.
 *
 * Consulta sobre una colección pública.
 */
class PublicQuery internal constructor(private val collection: PublicCollection) {
    private val filters = mutableListOf<PublicFilter>()
    private var order: String? = null
    private var descending = false
    private var max = 50

    /**
     * Operators: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza`, `existe`.
     *
     * Operadores: `==`, `!=`, `<`, `<=`, `>`, `>=`, `contiene`, `empieza`, `existe`.
     */
    fun where(field: String, operator: String, value: Any? = null) = apply {
        // El valor viaja como JSON suelto: 10, "pan", true.
        filters += PublicFilter(field, operator, JSONArray(listOf(value)).toString().removeSurrounding("[", "]"))
    }

    @Deprecated("Use where()", ReplaceWith("where(campo, operador, valor)"))
    fun donde(campo: String, operador: String, valor: Any? = null) = where(campo, operador, valor)

    /**
     * Sorts by `field`.
     *
     * Ordena por `field`.
     */
    fun sortBy(field: String, descending: Boolean = false) = apply { order = field; this.descending = descending }

    @Deprecated("Use sortBy()", ReplaceWith("sortBy(campo, descendente)"))
    fun ordenarPor(campo: String, descendente: Boolean = false) = sortBy(campo, descendente)

    /**
     * At most `n` documents (1-100; 50 by default).
     *
     * Como mucho `n` documentos (1–100; 50 por defecto).
     */
    fun limit(n: Int) = apply { max = n.coerceIn(1, 100) }

    @Deprecated("Use limit()", ReplaceWith("limit(n)"))
    fun limite(n: Int) = limit(n)

    /**
     * Runs the query over the network and reports the documents with `onDone`.
     *
     * Ejecuta la consulta por la red y entrega los documentos con `onDone`.
     */
    fun get(onDone: (Result<List<PublicDocument>>) -> Unit) = MobileMesh.inBackground({
        collection.mesh.core.publicQuery(collection.name, filters.toList(), order, descending, max.toUInt()).map { d ->
            val o = JSONObject(d.fields)
            PublicDocument(d.id, d.author, o.keys().asSequence().associateWith { k -> o.get(k).takeIf { it != JSONObject.NULL } })
        }
    }, onDone)

    @Deprecated("Use get()", ReplaceWith("get(alTerminar)"))
    fun obtener(alTerminar: (Result<List<PublicDocument>>) -> Unit) = get(alTerminar)
}

@Deprecated("Use PublicQuery")
typealias ConsultaPublica = PublicQuery
