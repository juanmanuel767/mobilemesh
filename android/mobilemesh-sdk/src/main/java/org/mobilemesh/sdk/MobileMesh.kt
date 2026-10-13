package org.mobilemesh.sdk

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import java.io.File
import java.net.Inet4Address
import java.net.NetworkInterface
import kotlin.concurrent.thread
import org.json.JSONArray
import uniffi.mobilemesh_core.IdentityState
import uniffi.mobilemesh_core.Mesh
import uniffi.mobilemesh_core.MeshException
import uniffi.mobilemesh_core.identityState
import uniffi.mobilemesh_core.parseInvite

/** Error de MobileMesh, con un mensaje en español listo para mostrar. */
class MobileMeshError(mensaje: String) : Exception(mensaje)

enum class EstadoCuenta {
    /** Primera vez: `iniciar` creará la cuenta. */
    SIN_CUENTA,

    /** Hay cuenta, pero el teléfono está bloqueado: se abre al desbloquearlo. */
    BLOQUEADO,

    /** Lista para usar. */
    LISTA,
}

/** Un documento de una colección. `conflictos`: valores en disputa de las colecciones estrictas. */
data class Documento(val id: String, val campos: Map<String, Any?>, val conflictos: Map<String, List<Any?>>)

/** Resultado de un envío: cuántos mensajes y si quedaron en el buzón del destinatario dormido. */
data class Envio(val cuantos: Int, val enBuzon: Boolean)

data class Mensaje(val id: String, val contacto: String, val enviado: Boolean, val texto: String, val estado: String, val archivo: String?)

/**
 * MobileMesh para tu app: base de datos, cuentas, mensajes, archivos, permisos y sincronización,
 * sin servidores. Los datos viven cifrados en el teléfono y viajan cifrados de extremo a extremo.
 *
 * ```
 * val mesh = MobileMesh.iniciar(context)
 * val pedidos = mesh.coleccion("pedidos")
 * pedidos.guardar("p1", mapOf("producto" to "pan", "cantidad" to 2))
 * pedidos.escuchar { docs -> mostrar(docs) }
 * ```
 *
 * Las operaciones de red (`enviarMensaje`, `enviarArchivo`, `sincronizarCon`) trabajan en segundo
 * plano y avisan con `alTerminar` en el hilo principal. El resto es inmediato.
 */
class MobileMesh private constructor(private val context: Context, val nucleo: Mesh) {

    companion object {
        private val ui = Handler(Looper.getMainLooper())

        fun estado(context: Context): EstadoCuenta {
            val guard = KeystoreGuard(context.applicationContext)
            return when (identityState(MeshHolder.dir(context), guard)) {
                IdentityState.NOTHING, IdentityState.PENDING -> EstadoCuenta.SIN_CUENTA
                IdentityState.READY -> if (MeshHolder.open(context) != null) EstadoCuenta.LISTA else EstadoCuenta.BLOQUEADO
            }
        }

        /**
         * Abre la cuenta del usuario (la crea la primera vez) y deja MobileMesh recibiendo en
         * segundo plano. `puerto`: distinto para cada app del mismo teléfono.
         * Falla con [MobileMeshError] si el teléfono está bloqueado.
         */
        fun iniciar(context: Context, puerto: Int = PUERTO_POR_DEFECTO): MobileMesh {
            val app = context.applicationContext
            MeshHolder.guardarPuerto(app, puerto)
            val m = MeshHolder.open(app) ?: run {
                if (estado(app) == EstadoCuenta.BLOQUEADO) throw MobileMeshError("El teléfono está bloqueado: desbloquéalo para abrir la cuenta")
                envolver { Mesh.create(MeshHolder.dir(app), KeystoreGuard(app)) }.also { MeshHolder.mesh = it }
            }
            MeshService.start(app)
            return MobileMesh(app, m)
        }

        /** Recupera una cuenta en un teléfono nuevo con su copia de recuperación. */
        fun recuperar(context: Context, codigo: String, contrasena: String, puerto: Int = PUERTO_POR_DEFECTO): MobileMesh {
            val app = context.applicationContext
            MeshHolder.guardarPuerto(app, puerto)
            val m = envolver { Mesh.restore(MeshHolder.dir(app), KeystoreGuard(app), codigo, contrasena) }
            MeshHolder.mesh = m
            MeshService.start(app)
            return MobileMesh(app, m)
        }

        internal fun <T> envolver(op: () -> T): T = try {
            op()
        } catch (e: MeshException.Fallo) {
            throw MobileMeshError(e.msg)
        }

        internal fun <T> enSegundoPlano(op: () -> T, alTerminar: (Result<T>) -> Unit) {
            thread {
                val r = runCatching { envolver(op) }
                ui.post { alTerminar(r) }
            }
        }
    }

    /** Identificador público de la cuenta. */
    val miId: String get() = nucleo.id()

    fun coleccion(nombre: String, estricta: Boolean = false) = Coleccion(this, nombre, estricta)

    // ---------- Contactos ----------

    /** Invitación (texto para un QR) con la que otro te añade. Solo lleva datos públicos. */
    fun invitacion(): String {
        val ip = ipLocal() ?: throw MobileMeshError("No hay conexión a ninguna red: conéctate a una wifi o a datos para crear tu invitación")
        return envolver { nucleo.invite("$ip:${MeshHolder.puerto(context)}") }
    }

    /** Añade un contacto con su invitación. Devuelve `true` si era otro dispositivo de tu cuenta. */
    fun anadirContacto(nombre: String, invitacion: String): Boolean = envolver {
        val i = parseInvite(invitacion)
        nucleo.addContact(nombre, i.key, i.addr)
    }

    fun contactos(): List<String> = envolver { nucleo.contacts().map { it.name } }

    fun bloquear(contacto: String, bloqueado: Boolean = true) = envolver { nucleo.setBlocked(contacto, bloqueado) }

    // ---------- Mensajes y archivos ----------

    /**
     * Envía un mensaje. Si el destino está dormido y tiene buzón en un relé, queda allí y lo recoge al
     * despertar (`enBuzon = true`); si no hay forma, queda pendiente y se reintenta solo.
     */
    fun enviarMensaje(contacto: String, texto: String, alTerminar: (Result<Envio>) -> Unit = {}) =
        enSegundoPlano({ nucleo.send(contacto, texto).let { Envio(it.count.toInt(), it.inMailbox) } }, alTerminar)

    fun mensajes(): List<Mensaje> = envolver {
        nucleo.messages().map { Mensaje(it.id, it.peer, it.outgoing, it.body, it.status, it.fileName) }
    }

    /** Envía un archivo elegido por el usuario (foto, vídeo, documento). Se guarda una copia cifrada. */
    fun enviarArchivo(contacto: String, archivo: Uri, nombre: String, alTerminar: (Result<Envio>) -> Unit = {}) =
        enSegundoPlano({
            val tmp = File(context.cacheDir, "salida-${System.nanoTime()}")
            try {
                context.contentResolver.openInputStream(archivo)!!.use { input -> tmp.outputStream().use { input.copyTo(it) } }
                val tipo = context.contentResolver.getType(archivo) ?: "application/octet-stream"
                nucleo.sendFile(contacto, tmp.path, nombre, tipo).let { Envio(it.count.toInt(), it.inMailbox) }
            } finally {
                tmp.delete()
            }
        }, alTerminar)

    /** Te avisa (en el hilo principal) cada vez que llega algo nuevo. Devuelve cómo dejar de escuchar. */
    fun alRecibir(oyente: (cuantos: Int) -> Unit): () -> Unit {
        MeshHolder.oyentes.add(oyente)
        return { MeshHolder.oyentes.remove(oyente) }
    }

    // ---------- Cuenta, red y diagnóstico ----------

    /** Copia de recuperación cifrada con `contrasena` (mínimo 10 caracteres). Solo el principal. */
    fun copiaDeRecuperacion(contrasena: String): String = envolver { nucleo.backup(contrasena) }

    /** Usa tu propio relé «host:puerto» para llegar a otras redes (se aplica al reiniciar la app). */
    fun usarRele(direccion: String?) = envolver { nucleo.setRelay(direccion) }

    fun modoSinConexion(activo: Boolean) = nucleo.setOffline(activo)

    /**
     * Cada cuántos minutos mira el buzón con el teléfono dormido (1–60; 3 por defecto). Menos minutos:
     * avisos antes, algo más de batería y datos. Android lo espacia por su cuenta en reposo profundo.
     */
    fun mirarBuzonCada(minutos: Int) = Despertador.cambiarMinutos(context, minutos)

    /**
     * Recepción instantánea sin Google, también con la pantalla apagada en teléfonos que congelan las
     * apps (TECNO, ZTE…). Gasta más batería (mantiene el sonido abierto, en silencio): mejor que la
     * active el usuario. Desactivarla surte efecto al reiniciar la app.
     */
    fun recepcionInstantanea(activa: Boolean) = Silencio.activar(context, activa)

    /** Resumen del estado para depurar (sin datos privados). */
    fun diagnostico(): String = envolver { nucleo.diagnostics() }

    private fun ipLocal(): String? = NetworkInterface.getNetworkInterfaces().toList()
        .flatMap { it.inetAddresses.toList() }
        .firstOrNull { it is Inet4Address && !it.isLoopbackAddress }?.hostAddress
}

/**
 * Una colección de documentos. Cada campo se guarda cifrado en el teléfono y se sincroniza con tus
 * otros dispositivos y con quien la compartas. `estricta`: los cambios hechos a la vez en dos sitios
 * no se mezclan solos (para dinero, inventario…), quedan en `Documento.conflictos` hasta elegir.
 */
class Coleccion internal constructor(private val mesh: MobileMesh, val nombre: String, private val estricta: Boolean) {

    /** Guarda campos de un documento (texto, números, booleanos…). Un valor `null` borra ese campo. */
    fun guardar(id: String, campos: Map<String, Any?>) = MobileMesh.envolver {
        campos.forEach { (campo, valor) -> mesh.nucleo.put(nombre, id, campo, valor?.let { JSONArray().put(it).toString() }, estricta) }
    }

    fun leer(id: String): Documento? = todos().firstOrNull { it.id == id }

    fun todos(): List<Documento> = MobileMesh.envolver {
        mesh.nucleo.records(nombre).map { r ->
            Documento(
                r.id,
                r.fields.associate { it.name to valor(it.value) },
                r.conflicts.associate { c -> c.field to c.values.map(::valor) },
            )
        }
    }

    /** Borra un documento (en todos los dispositivos al sincronizar). */
    fun borrar(id: String) {
        leer(id)?.let { doc -> guardar(id, doc.campos.mapValues { null }) }
    }

    /**
     * Llama a `oyente` con todos los documentos ahora y cada vez que cambien (por ti, por tus otros
     * dispositivos o por quien la compartas). Devuelve cómo dejar de escuchar.
     */
    // ponytail: comprueba cada segundo; cambiar por un aviso del núcleo si hace falta más inmediatez.
    fun escuchar(oyente: (List<Documento>) -> Unit): () -> Unit {
        val ui = Handler(Looper.getMainLooper())
        var ultimo: List<Documento>? = null
        var activo = true
        fun mirar() {
            if (!activo) return
            val ahora = runCatching { todos() }.getOrNull()
            if (ahora != null && ahora != ultimo) {
                ultimo = ahora
                oyente(ahora)
            }
            ui.postDelayed(::mirar, 1000)
        }
        mirar()
        return { activo = false }
    }

    /** Comparte la colección con un contacto. `puedeCambiar = false`: solo puede leerla. */
    fun compartirCon(contacto: String, puedeCambiar: Boolean) = MobileMesh.envolver { mesh.nucleo.share(nombre, contacto, puedeCambiar) }

    fun dejarDeCompartirCon(contacto: String) = MobileMesh.envolver { mesh.nucleo.share(nombre, contacto, null) }

    /** Sincroniza ahora con un contacto (también se hace solo cada pocos segundos). */
    fun sincronizarCon(contacto: String, alTerminar: (Result<String>) -> Unit = {}) =
        MobileMesh.enSegundoPlano({ mesh.nucleo.syncWithContact(contacto) }, alTerminar)

    private fun valor(guardado: String): Any? = runCatching { JSONArray(guardado).get(0) }.getOrDefault(guardado)
}
