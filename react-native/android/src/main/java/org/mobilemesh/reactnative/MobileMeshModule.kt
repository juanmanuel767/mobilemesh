package org.mobilemesh.reactnative

import android.app.Activity
import android.content.ContentValues
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.provider.OpenableColumns
import com.facebook.react.bridge.*
import com.facebook.react.modules.core.DeviceEventManagerModule
import org.json.JSONArray
import org.json.JSONObject
import org.mobilemesh.sdk.EstadoCuenta
import org.mobilemesh.sdk.MobileMesh
import org.mobilemesh.sdk.Permisos
import java.io.File
import kotlin.concurrent.thread

/** Puente de MobileMesh para React Native: expone el SDK de Android a JavaScript. */
class MobileMeshModule(private val ctx: ReactApplicationContext) : ReactContextBaseJavaModule(ctx), ActivityEventListener {
    private var mesh: MobileMesh? = null
    private var dejarDeEscuchar: (() -> Unit)? = null
    private var eligiendo: Promise? = null

    init {
        ctx.addActivityEventListener(this)
    }

    override fun getName() = "MobileMesh"

    private fun m() = mesh ?: throw IllegalStateException("Llama antes a MobileMesh.iniciar()")

    /** Ejecuta en otro hilo (el núcleo usa red y disco) y resuelve la promesa con el resultado. */
    // Devuelve Unit: React Native exige que los métodos con promesa no devuelvan nada.
    private fun trabajar(p: Promise, fn: () -> Any?) {
        thread {
            try {
                p.resolve(fn())
            } catch (e: Throwable) {
                p.reject("MobileMesh", e.message ?: e.toString(), e)
            }
        }
    }

    private fun emitir(evento: String, datos: Any?) {
        ctx.getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java).emit(evento, datos)
    }

    private fun preparar(nuevo: MobileMesh): String {
        mesh = nuevo
        if (dejarDeEscuchar == null) dejarDeEscuchar = nuevo.alRecibir { n -> emitir("MobileMeshNuevos", n) }
        ctx.currentActivity?.let { a -> a.runOnUiThread { Permisos.pedir(a) } } // avisos, batería, alarmas
        return nuevo.miId
    }

    /** "sin cuenta" | "bloqueado" | "lista" */
    @ReactMethod
    fun estadoCuenta(p: Promise) = trabajar(p) {
        when (MobileMesh.estado(ctx)) {
            EstadoCuenta.SIN_CUENTA -> "sin cuenta"
            EstadoCuenta.BLOQUEADO -> "bloqueado"
            EstadoCuenta.LISTA -> "lista"
        }
    }

    // Puerto propio por app: dos apps de MobileMesh en el mismo móvil no pueden compartirlo (si no,
    // contesta la otra y los mensajes se rechazan por «posible suplantación»).
    private val puerto = 7812 + Math.floorMod(ctx.packageName.hashCode(), 256)

    @ReactMethod
    fun iniciar(p: Promise) = trabajar(p) { preparar(mesh ?: MobileMesh.iniciar(ctx, puerto)) }

    // ---------- Usuarios por nombre ----------

    private fun prefs() = ctx.getSharedPreferences("mobilemesh-rn", 0)

    /** Mi nombre de usuario guardado en este móvil (o null). */
    @ReactMethod fun miUsuario(p: Promise) = trabajar(p) { prefs().getString("usuario", null) }

    @ReactMethod fun usarGuiaDeNombres(invitacion: String?, p: Promise) = trabajar(p) { m().usarGuiaDeNombres(invitacion); null }

    /**
     * Crea la cuenta con un nombre de usuario y una contraseña: registra el nombre en la guía y
     * guarda la cuenta en el guardián para poder entrar desde otro móvil. Responde un aviso o null.
     */
    @ReactMethod
    fun registrar(usuario: String, contrasena: String, p: Promise) = trabajar(p) {
        val mm = m()
        if (mm.existeUsuario(usuario)) throw IllegalStateException("El usuario «$usuario» ya existe")
        mm.registrarNombre(usuario)
        prefs().edit().putString("usuario", usuario.trim().lowercase()).apply()
        // El guardián es para recuperar la cuenta; si no responde, la cuenta funciona igual.
        runCatching { mm.protegerConGuardian(usuario, contrasena); null }.getOrElse { "Cuenta creada, pero sin copia en el guardián: ${it.message}" }
    }

    /** Entrar en un móvil nuevo con usuario y contraseña (recupera la cuenta del guardián). */
    @ReactMethod
    fun entrar(usuario: String, contrasena: String, p: Promise) = trabajar(p) {
        val id = preparar(MobileMesh.recuperarConGuardian(ctx, usuario, contrasena, puerto).first)
        prefs().edit().putString("usuario", usuario.trim().lowercase()).apply()
        id
    }

    /**
     * Vuelve a apuntar mi invitación en la guía (cambia al cambiar de red), acepta solicitudes y
     * trae la dirección actual de mis contactos (también cambia al cambiar de red).
     */
    @ReactMethod
    fun actualizar(p: Promise) = trabajar(p) {
        val mm = m()
        prefs().getString("usuario", null)?.let { mm.registrarNombre(it) }
        val nuevos = mm.aceptarSolicitudes()
        // Solo actualiza la dirección: si la clave no coincide, anadirContacto lo rechaza.
        for (c in mm.contactos()) runCatching { mm.nucleo.findName(c)?.let { mm.anadirContacto(c, it) } }
        Arguments.fromList(nuevos)
    }

    @ReactMethod fun existeUsuario(usuario: String, p: Promise) = trabajar(p) { m().existeUsuario(usuario) }

    @ReactMethod fun anadirPorNombre(usuario: String, p: Promise) = trabajar(p) { m().anadirPorNombre(usuario); null }

    @ReactMethod fun aceptarSolicitudes(p: Promise) = trabajar(p) { Arguments.fromList(m().aceptarSolicitudes()) }

    @ReactMethod fun invitacion(p: Promise) = trabajar(p) { m().invitacion() }

    @ReactMethod fun anadirContacto(nombre: String, invitacion: String, p: Promise) = trabajar(p) { m().anadirContacto(nombre, invitacion) }

    @ReactMethod fun contactos(p: Promise) = trabajar(p) { Arguments.fromList(m().contactos()) }

    @ReactMethod fun usarRele(direccion: String?, p: Promise) = trabajar(p) { m().usarRele(direccion); null }

    @ReactMethod fun diagnostico(p: Promise) = trabajar(p) { m().diagnostico() }

    @ReactMethod
    fun enviar(contacto: String, texto: String, p: Promise) =
        trabajar(p) { m().nucleo.send(contacto, texto).let { if (it.inMailbox) "en buzón" else "entregado" } }

    @ReactMethod
    fun mensajes(p: Promise) = trabajar(p) {
        val lista = JSONArray()
        m().nucleo.messages().forEach {
            lista.put(
                JSONObject()
                    .put("id", it.id).put("contacto", it.peer).put("enviado", it.outgoing)
                    .put("texto", it.body).put("estado", it.status).put("progreso", it.progress.toInt())
                    .put("archivo", it.fileName ?: JSONObject.NULL).put("tipo", it.mime ?: JSONObject.NULL)
                    .put("tamano", it.fileSize?.toLong() ?: JSONObject.NULL).put("disponible", it.fileAvailable),
            )
        }
        lista.toString()
    }

    // ---------- Archivos ----------

    /** Abre el selector de archivos de Android. Resuelve {uri, nombre} o null si se cancela. */
    @ReactMethod
    fun elegirArchivo(p: Promise) {
        val a = ctx.currentActivity ?: return p.reject("MobileMesh", "No hay pantalla abierta")
        eligiendo = p
        a.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*"), ELEGIR)
    }

    override fun onActivityResult(activity: Activity, requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode != ELEGIR) return
        val p = eligiendo.also { eligiendo = null } ?: return
        val uri = data?.data
        if (resultCode != Activity.RESULT_OK || uri == null) return p.resolve(null)
        p.resolve(Arguments.createMap().apply { putString("uri", uri.toString()); putString("nombre", nombreDe(uri)) })
    }

    override fun onNewIntent(intent: Intent) {}

    private fun nombreDe(uri: Uri): String =
        ctx.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        } ?: uri.lastPathSegment ?: "archivo"

    @ReactMethod
    fun enviarArchivo(contacto: String, uri: String, nombre: String, p: Promise) {
        m().enviarArchivo(contacto, Uri.parse(uri), nombre) { r ->
            r.fold({ p.resolve(if (it.enBuzon) "en buzón" else "entregado") }, { p.reject("MobileMesh", it.message, it) })
        }
    }

    private fun mensaje(id: String) = m().nucleo.messages().first { it.id == id }

    /** Descifra un archivo a la caché de la app y devuelve su ruta `file://` (para ver fotos). */
    @ReactMethod
    fun verArchivo(id: String, p: Promise) = trabajar(p) {
        val msg = mensaje(id)
        val f = File(ctx.cacheDir, "ver-${msg.id}")
        if (!f.exists()) m().nucleo.exportFile(msg.peerKey, msg.outgoing, msg.id, f.path)
        "file://${f.path}"
    }

    /** Descifra un archivo y lo deja en Descargas. */
    @ReactMethod
    fun guardarEnDescargas(id: String, p: Promise) = trabajar(p) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) throw IllegalStateException("Necesita Android 10 o superior")
        val msg = mensaje(id)
        val tmp = File(ctx.cacheDir, "exporta-${msg.id}")
        try {
            m().nucleo.exportFile(msg.peerKey, msg.outgoing, msg.id, tmp.path)
            val valores = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, msg.fileName)
                put(MediaStore.Downloads.MIME_TYPE, msg.mime)
            }
            val destino = ctx.contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, valores)!!
            ctx.contentResolver.openOutputStream(destino)!!.use { out -> tmp.inputStream().use { it.copyTo(out) } }
            msg.fileName
        } finally {
            tmp.delete()
        }
    }

    // ---------- Base de datos (colecciones cifradas) ----------

    @ReactMethod
    fun guardar(coleccion: String, id: String, campos: ReadableMap, p: Promise) =
        trabajar(p) { m().coleccion(coleccion).guardar(id, campos.toHashMap()); null }

    @ReactMethod
    fun datos(coleccion: String, p: Promise) = trabajar(p) {
        JSONArray(m().coleccion(coleccion).todos().map { JSONObject().put("id", it.id).put("campos", JSONObject(it.campos)) }).toString()
    }

    @ReactMethod
    fun borrar(coleccion: String, id: String, p: Promise) = trabajar(p) { m().coleccion(coleccion).borrar(id); null }

    // Necesarios para NativeEventEmitter.
    @ReactMethod fun addListener(evento: String) {}

    @ReactMethod fun removeListeners(cuantos: Double) {}

    companion object {
        private const val ELEGIR = 7841
    }
}
