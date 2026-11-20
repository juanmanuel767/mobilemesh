package org.mobilemesh.flutter

import android.app.Activity
import android.content.Context
import android.os.Handler
import android.os.Looper
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.embedding.engine.plugins.activity.ActivityAware
import io.flutter.embedding.engine.plugins.activity.ActivityPluginBinding
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import org.mobilemesh.sdk.Document
import org.mobilemesh.sdk.Message
import org.mobilemesh.sdk.MobileMesh
import org.mobilemesh.sdk.Permissions
import org.mobilemesh.sdk.Query
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/**
 * Puente de MobileMesh para Flutter: expone el SDK de Android a Dart por el canal «mobilemesh».
 *
 * El trabajo pesado (red y disco) se hace en un hilo aparte y la respuesta se devuelve en el hilo
 * principal, igual que en el puente de React Native.
 */
class MobileMeshPlugin : FlutterPlugin, MethodChannel.MethodCallHandler, ActivityAware {
    private lateinit var canal: MethodChannel
    private lateinit var contexto: Context
    private var actividad: Activity? = null
    private var mesh: MobileMesh? = null
    private val principal = Handler(Looper.getMainLooper())

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        contexto = binding.applicationContext
        canal = MethodChannel(binding.binaryMessenger, "mobilemesh")
        canal.setMethodCallHandler(this)
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        canal.setMethodCallHandler(null)
    }

    override fun onAttachedToActivity(binding: ActivityPluginBinding) {
        actividad = binding.activity
    }

    override fun onDetachedFromActivityForConfigChanges() {
        actividad = null
    }

    override fun onReattachedToActivityForConfigChanges(binding: ActivityPluginBinding) {
        actividad = binding.activity
    }

    override fun onDetachedFromActivity() {
        actividad = null
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        try {
            when (call.method) {
                "start" -> trabajar(result) { iniciar() }
                "accountId" -> trabajar(result) { m().myId }
                "save" -> trabajar(result) { guardar(call); null }
                "all" -> trabajar(result) { documentos(call) }
                "query" -> trabajar(result) { consultar(call) }
                "invitation" -> trabajar(result) { m().invitation() }
                "addContact" -> trabajar(result) {
                    m().addContact(argumento(call, "name"), argumento(call, "invitation"))
                }
                "contacts" -> trabajar(result) { m().contacts() }
                "sendMessage" -> trabajar(result) { enviar(call) }
                "messages" -> trabajar(result) { m().messages().map(::mensaje) }
                "shareWith" -> trabajar(result) { compartir(call); null }
                "syncWith" -> trabajar(result) { sincronizar(call) }
                else -> result.notImplemented()
            }
        } catch (e: Throwable) {
            result.error("mobilemesh", e.message ?: e.toString(), null)
        }
    }

    /** Ejecuta en otro hilo (el núcleo usa red y disco) y responde en el hilo principal. */
    private fun trabajar(result: MethodChannel.Result, fn: () -> Any?) {
        thread {
            try {
                val salida = fn()
                principal.post { result.success(salida) }
            } catch (e: Throwable) {
                val mensaje = e.message ?: e.toString()
                principal.post { result.error("mobilemesh", mensaje, null) }
            }
        }
    }

    private fun m(): MobileMesh =
        mesh ?: throw IllegalStateException("Llama antes a MobileMesh.start()")

    /**
     * Abre la cuenta (la crea la primera vez) y pide los permisos de avisos. El `appId` de Android
     * sale de `assets/mobilemesh.properties` o del nombre del paquete, no se configura en runtime.
     */
    private fun iniciar(): String {
        val mm = mesh ?: MobileMesh.start(contexto, puerto()).also { mesh = it }
        actividad?.let { a -> a.runOnUiThread { Permissions.request(a) } }
        return mm.myId
    }

    // Puerto propio por app: dos apps de MobileMesh en el mismo móvil no pueden compartirlo.
    private fun puerto() = 7812 + Math.floorMod(contexto.packageName.hashCode(), 256)

    private inline fun <reified T> argumento(call: MethodCall, nombre: String): T =
        call.argument<T>(nombre) ?: throw IllegalArgumentException("Falta el argumento «$nombre»")

    private fun guardar(call: MethodCall) {
        val campos = call.argument<Map<String, Any?>>("fields")
            ?: throw IllegalArgumentException("Falta el argumento «fields»")
        m().collection(argumento(call, "collection")).save(argumento(call, "id"), campos)
    }

    private fun documentos(call: MethodCall): List<Map<String, Any?>> =
        m().collection(argumento(call, "collection")).all().map(::documento)

    private fun documento(d: Document): Map<String, Any?> =
        mapOf("id" to d.id, "fields" to d.fields)

    /** `options`: { donde/where: [[campo, operador, valor]], orden/order, desc, limite/limit }. */
    private fun consultar(call: MethodCall): List<Map<String, Any?>> {
        val coleccion = m().collection(argumento(call, "collection"))
        val opciones = call.argument<Map<String, Any?>>("options") ?: emptyMap()
        val donde = opciones["where"] as? List<*> ?: emptyList<Any?>()
        val orden = opciones["order"] as? String
        val descendente = opciones["desc"] as? Boolean ?: false
        val limite = (opciones["limit"] as? Number)?.toInt()

        var consulta: Query? = null
        if (donde.isNotEmpty()) {
            val primero = donde.first() as List<*>
            var q = coleccion.where(primero[0] as String, primero[1] as String, primero.getOrNull(2))
            for (filtro in donde.drop(1)) {
                val f = filtro as List<*>
                q = q.where(f[0] as String, f[1] as String, f.getOrNull(2))
            }
            consulta = q
        }
        if (orden != null) {
            consulta = consulta?.sortBy(orden, descendente) ?: coleccion.sortBy(orden, descendente)
        }

        val q = consulta ?: return coleccion.all()
            .let { if (limite != null) it.take(limite) else it }
            .map(::documento)
        val conLimite = if (limite != null) q.limit(limite) else q
        return conLimite.get().map(::documento)
    }

    private fun enviar(call: MethodCall): String {
        val entrega = esperar { alTerminar ->
            m().sendMessage(argumento(call, "contact"), argumento(call, "text"), alTerminar)
        }
        return if (entrega.inMailbox) "en buzón" else "entregado"
    }

    private fun compartir(call: MethodCall) {
        m().collection(argumento(call, "collection"))
            .shareWith(argumento(call, "contact"), call.argument<Boolean>("canWrite") ?: false)
    }

    /**
     * `syncWith` está en la colección en Kotlin, pero solo usa el núcleo y sincroniza todo lo
     * compartido con ese contacto; el nombre de la colección no influye.
     */
    private fun sincronizar(call: MethodCall): String =
        esperar { alTerminar -> m().collection("").syncWith(argumento(call, "contact"), alTerminar) }

    private fun mensaje(msg: Message): Map<String, Any?> = mapOf(
        "id" to msg.id,
        "contact" to msg.contact,
        "sent" to msg.sent,
        "text" to msg.text,
        "status" to msg.status,
        "file" to msg.file,
    )

    /** Espera el resultado de una llamada asíncrona del SDK (enviar, sincronizar…). */
    private fun <T> esperar(arrancar: ((Result<T>) -> Unit) -> Unit): T {
        val listo = CountDownLatch(1)
        var salida: Result<T>? = null
        arrancar { r ->
            salida = r
            listo.countDown()
        }
        if (!listo.await(60, TimeUnit.SECONDS)) {
            throw IllegalStateException("La operación tardó demasiado (60 s)")
        }
        return (salida ?: throw IllegalStateException("Sin respuesta del SDK")).getOrThrow()
    }
}
