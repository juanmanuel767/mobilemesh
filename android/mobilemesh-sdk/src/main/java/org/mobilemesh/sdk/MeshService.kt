package org.mobilemesh.sdk

import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.os.Build
import uniffi.mobilemesh_core.Events
import uniffi.mobilemesh_core.IdentityState
import uniffi.mobilemesh_core.Mesh
import uniffi.mobilemesh_core.identityState

/** Puerto por defecto; cada app puede elegir el suyo en `MobileMesh.iniciar` (dos apps no pueden compartirlo). */
const val PUERTO_POR_DEFECTO = 7801
private const val ONGOING = 1
private const val NEW_MESSAGE = 2

/** Una sola identidad abierta por proceso, compartida por la pantalla y el servicio. */
object MeshHolder {
    @Volatile var mesh: Mesh? = null

    /** Quien quiere enterarse de que ha llegado algo nuevo (se llama en el hilo principal). */
    val oyentes = java.util.concurrent.CopyOnWriteArrayList<(Int) -> Unit>()

    fun puerto(c: Context): Int = c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE).getInt("puerto", PUERTO_POR_DEFECTO)

    fun guardarPuerto(c: Context, puerto: Int) =
        c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE).edit().putInt("puerto", puerto).apply()

    fun dir(c: Context) = c.filesDir.resolve("mesh").path

    /** Abre la identidad si se puede; `null` si no hay o si el teléfono está bloqueado. */
    @Synchronized
    fun open(c: Context): Mesh? {
        mesh?.let { return it }
        val guard = KeystoreGuard(c.applicationContext)
        if (identityState(dir(c), guard) != IdentityState.READY) return null
        return runCatching { Mesh.open(dir(c), guard) }.getOrNull()?.also { mesh = it }
    }
}

/**
 * Mantiene MobileMesh conectado aunque la app esté cerrada (Fase 11): escucha directa, buzón en el
 * relé, reintentos y sincronización. Android exige una notificación fija para permitirlo.
 * Los avisos de mensajes nuevos son locales (no pasan por ningún servidor) y no llevan el contenido.
 */
class MeshService : Service() {
    private var started = false

    override fun onBind(intent: Intent?) = null

    override fun onCreate() {
        super.onCreate()
        val nm = getSystemService(NotificationManager::class.java)
        // Visible (aunque silenciosa): algunos sistemas (p. ej. HiOS de TECNO) congelan las apps cuya
        // notificación fija está oculta, y entonces no reciben nada.
        nm.deleteNotificationChannel("conexion")
        nm.createNotificationChannel(NotificationChannel("conexion-visible", "Conexión de ${Permisos.nombreApp(this)}", NotificationManager.IMPORTANCE_LOW))
        nm.createNotificationChannel(NotificationChannel("mensajes", "Mensajes nuevos", NotificationManager.IMPORTANCE_HIGH))
        val n = ongoing("Conectando…")
        if (Build.VERSION.SDK_INT >= 34) startForeground(ONGOING, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING)
        else startForeground(ONGOING, n)
        // Recepción instantánea (opcional): con audio en silencio el fabricante no congela la app.
        if (Silencio.activo(this)) Silencio.empezar()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        start()
        // Cada vez que se abre la app: recoger ya lo que esté en el buzón (el sistema pudo tenerla
        // congelada sin dejarla despertar).
        MeshHolder.mesh?.let { m ->
            Thread { runCatching { m.checkMailbox().toInt() }.getOrNull()?.takeIf { it > 0 }?.let { Avisos.mensajeNuevo(this, it) } }.start()
        }
        return START_STICKY // si Android lo cierra, lo vuelve a arrancar
    }

    private fun start() {
        if (started) return
        val m = MeshHolder.open(this) ?: return waitForUnlock()
        try {
            m.listen(MeshHolder.puerto(this).toUShort(), events)
        } catch (e: Exception) {
            // El puerto ya está abierto en este proceso: no pasa nada.
        }
        m.startBackground(15u, events)
        Despertador.programar(this)
        started = true
        update("Conectado: recibes mensajes aunque la app esté cerrada")
    }

    /** La identidad solo se abre con el teléfono desbloqueado: se espera al primer desbloqueo. */
    private fun waitForUnlock() {
        update("Desbloquea el teléfono para empezar a recibir mensajes")
        registerReceiver(object : BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) {
                unregisterReceiver(this)
                start()
            }
        }, IntentFilter(Intent.ACTION_USER_PRESENT))
    }

    private val events = object : Events {
        override fun received(count: UInt) = Avisos.mensajeNuevo(this@MeshService, count.toInt())
    }

    private fun ongoing(text: String) = Notification.Builder(this, "conexion-visible")
        .setSmallIcon(android.R.drawable.stat_notify_sync_noanim)
        .setContentTitle(Permisos.nombreApp(this))
        .setContentText(text)
        .setOngoing(true)
        .setContentIntent(Avisos.abrirApp(this))
        .build()

    private fun update(text: String) = getSystemService(NotificationManager::class.java).notify(ONGOING, ongoing(text))

    companion object {
        fun start(c: Context) {
            c.startForegroundService(Intent(c, MeshService::class.java))
        }
    }
}

/**
 * Recepción instantánea sin Google (opcional): reproduce silencio sin parar (8 kHz, mono). Los
 * fabricantes no congelan una app que suena, así que la conexión sigue viva con la pantalla apagada
 * (medido en un TECNO: mensaje entregado en 1 s). Cuesta batería: el chip de sonido no se duerme.
 */
object Silencio {
    private var hilo: Thread? = null

    fun activo(c: Context) = c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE).getBoolean("recepcion_instantanea", false)

    fun activar(c: Context, si: Boolean) {
        c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE).edit().putBoolean("recepcion_instantanea", si).apply()
        if (si) empezar() // ponytail: desactivar surte efecto al reiniciar la app
    }

    fun empezar() {
        if (hilo != null) return
        hilo = Thread {
            val tam = android.media.AudioTrack.getMinBufferSize(8000, android.media.AudioFormat.CHANNEL_OUT_MONO, android.media.AudioFormat.ENCODING_PCM_16BIT)
            val pista = android.media.AudioTrack.Builder()
                .setAudioAttributes(android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_MEDIA).setContentType(android.media.AudioAttributes.CONTENT_TYPE_MUSIC).build())
                .setAudioFormat(android.media.AudioFormat.Builder().setSampleRate(8000).setChannelMask(android.media.AudioFormat.CHANNEL_OUT_MONO).setEncoding(android.media.AudioFormat.ENCODING_PCM_16BIT).build())
                .setBufferSizeInBytes(tam * 4)
                .build()
            val ceros = ShortArray(tam)
            pista.play()
            while (true) pista.write(ceros, 0, ceros.size)
        }.apply { isDaemon = true; start() }
    }
}

/** Avisos locales de mensajes nuevos: nunca llevan el contenido. */
object Avisos {
    fun abrirApp(c: Context): PendingIntent = PendingIntent.getActivity(
        c, 0, c.packageManager.getLaunchIntentForPackage(c.packageName) ?: Intent(), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    fun mensajeNuevo(c: Context, count: Int) {
        android.os.Handler(c.mainLooper).post { MeshHolder.oyentes.forEach { it(count) } }
        CopiaAutomatica.cambio(c)
        val nm = c.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel("mensajes", "Mensajes nuevos", NotificationManager.IMPORTANCE_HIGH))
        val from = MeshHolder.mesh?.messages()?.lastOrNull { !it.outgoing }?.peer ?: "un contacto"
        // En la pantalla de bloqueo solo se ve que hay un mensaje, no de quién ni qué dice.
        val public = Notification.Builder(c, "mensajes")
            .setSmallIcon(android.R.drawable.stat_notify_chat)
            .setContentTitle(Permisos.nombreApp(c))
            .setContentText("Tienes un mensaje nuevo")
            .build()
        val n = Notification.Builder(c, "mensajes")
            .setSmallIcon(android.R.drawable.stat_notify_chat)
            .setContentTitle("Mensaje nuevo")
            .setContentText(if (count > 1) "$count mensajes nuevos de $from" else "De $from")
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(public)
            .setContentIntent(abrirApp(c))
            .setAutoCancel(true)
            .build()
        nm.notify(NEW_MESSAGE, n)
    }
}

/**
 * Despertador (avisos en frío, sin Google): cada pocos minutos (3 por defecto) Android despierta a
 * MobileMesh aunque el teléfono esté en reposo y la app congelada. Mira el buzón del relé, guarda lo
 * nuevo, avisa y se vuelve a programar. En reposo profundo Android lo espacia por su cuenta (unos
 * 9–10 minutos). Cada despertar es una conexión corta (~3 KB).
 */
object Despertador {
    const val MINUTOS_POR_DEFECTO = 3

    /** Segundos entre despertares (se guarda en segundos para poder medir con intervalos cortos). */
    fun segundos(c: Context) = c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE).getInt("despertar_seg", MINUTOS_POR_DEFECTO * 60)

    fun cambiarMinutos(c: Context, minutos: Int) {
        c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE).edit().putInt("despertar_seg", minutos.coerceIn(1, 60) * 60).apply()
        programar(c)
    }

    fun programar(c: Context) {
        val am = c.getSystemService(AlarmManager::class.java)
        val pi = PendingIntent.getBroadcast(c, 1, Intent(c, DespertadorReceiver::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val cuando = android.os.SystemClock.elapsedRealtime() + segundos(c) * 1000L
        // Con el permiso «Alarmas y recordatorios», el despertador exacto: Android lo retrasa menos
        // con la pantalla apagada. Sin él, el flexible (Android lo junta con otros y lo retrasa).
        if (Build.VERSION.SDK_INT < 31 || am.canScheduleExactAlarms()) {
            am.setExactAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, cuando, pi)
        } else {
            am.setAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, cuando, pi)
        }
    }
}

class DespertadorReceiver : BroadcastReceiver() {
    override fun onReceive(c: Context, i: Intent) {
        val app = c.applicationContext
        android.util.Log.i("MobileMesh", "despertador: mirando el buzón")
        Despertador.programar(app)
        val pendiente = goAsync() // Android da unos segundos para terminar
        Thread {
            try {
                // Si el proceso se reinició y el teléfono está bloqueado no se puede abrir la cuenta:
                // se intentará en el siguiente despertar.
                val m = MeshHolder.open(app)
                val n = m?.checkMailbox()?.toInt() ?: 0
                if (n > 0) Avisos.mensajeNuevo(app, n)
                CopiaAutomatica.siToca(app)
            } catch (e: Exception) {
                // Sin red o relé caído: se reintenta en el siguiente despertar.
            } finally {
                pendiente.finish()
            }
        }.start()
    }
}

/**
 * Copia automática: vuelve a subir la copia cifrada (contactos, mensajes y datos) a quien la guarda
 * —el guardián, si se usó `protegerConGuardian`—. Tras cada cambio (mensaje enviado o recibido, dato
 * guardado) se hace al minuto, como mucho cada `HUECO_MIN` minutos; y aunque no haya cambios, cada
 * `CADA_HORAS`. Si el proceso muere antes, el despertador la hace en su siguiente vuelta. Así, al
 * formatear o perder el teléfono, se recupera lo hablado hasta hace un par de minutos. Sin nadie
 * que guarde la copia, no hace nada.
 */
object CopiaAutomatica {
    const val CADA_HORAS = 6
    const val HUECO_MIN = 2 // el guardián admite 20 copias por minuto: de sobra
    private const val ESPERA_MS = 60_000L
    private val enMarcha = java.util.concurrent.atomic.AtomicBoolean(false)
    private val programada = java.util.concurrent.atomic.AtomicBoolean(false)

    private fun prefs(c: Context) = c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE)

    /** Hubo un cambio: la copia se hará en un minuto (varios cambios seguidos, una sola copia). */
    fun cambio(c: Context) {
        val app = c.applicationContext
        prefs(app).edit().putBoolean("copia_pendiente", true).apply()
        if (!programada.compareAndSet(false, true)) return
        Thread {
            Thread.sleep(ESPERA_MS)
            programada.set(false)
            siToca(app)
        }.start()
    }

    fun siToca(c: Context) {
        val p = prefs(c)
        val edad = System.currentTimeMillis() - p.getLong("ultima_copia", 0)
        val toca = (p.getBoolean("copia_pendiente", false) && edad >= HUECO_MIN * 60_000L) || edad >= CADA_HORAS * 3_600_000L
        val m = MeshHolder.mesh
        if (!toca || m == null || !enMarcha.compareAndSet(false, true)) return
        Thread {
            // Se marca antes de copiar: un cambio durante la copia la vuelve a dejar pendiente.
            p.edit().putBoolean("copia_pendiente", false).apply()
            try {
                m.backupNow()
                p.edit().putLong("ultima_copia", System.currentTimeMillis()).apply()
                android.util.Log.i("MobileMesh", "copia automática hecha")
            } catch (e: Exception) {
                // Nadie guarda la copia («elige primero…») o sin red: queda pendiente.
                p.edit().putBoolean("copia_pendiente", true).apply()
                android.util.Log.i("MobileMesh", "copia automática pendiente: ${e.message}")
            } finally {
                enMarcha.set(false)
            }
        }.start()
    }
}

/**
 * Al encender el teléfono o al actualizar la app (Android borra sus alarmas), arranca el servicio
 * (esperará al primer desbloqueo) y vuelve a programar el despertador.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(c: Context, i: Intent) {
        if (i.action == Intent.ACTION_BOOT_COMPLETED || i.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
            MeshService.start(c)
            Despertador.programar(c)
        }
    }
}
