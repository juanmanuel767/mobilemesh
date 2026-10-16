package org.mobilemesh.rele

import android.annotation.SuppressLint
import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.widget.TextView
import java.io.File
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.HttpURLConnection
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface
import java.net.URL

/** Puerto y límite de conexiones del relé. */
private const val PUERTO = 7900
// --limite: el relé nunca usa más de 500 KB/s (~4 Mbit/s) para que el internet de casa vaya bien.
// --websocket: puente para navegadores (apps web) en su propio puerto.
private const val PUERTO_WEB = 7901
private val ARGS = listOf("0.0.0.0:$PUERTO", "--max", "5000", "--limite", "500", "--websocket", "0.0.0.0:$PUERTO_WEB")

/** Guardián (pieza aparte: su programa, su carpeta, su puerto y su registro). */
private const val PUERTO_GUARDIAN = 7954

/** Relés universales a los que preguntar «¿con qué IP me ves?» para saber mi IP pública. */
private val RELES_QUIEN = listOf("181.60.197.125" to 7900)

/** Al encender el móvil (o al actualizar la app) arranca el relé. */
class Arranque : BroadcastReceiver() {
    override fun onReceive(c: Context, i: Intent) {
        c.startForegroundService(Intent(c, ReleService::class.java))
    }
}

/** Mantiene en marcha el relé y el guardián, cada uno en su propio proceso. */
class ReleService : Service() {
    private var hilo: Thread? = null

    override fun onBind(intent: Intent?) = null

    @SuppressLint("WakelockTimeout") // a propósito: el relé tiene que estar siempre despierto
    override fun onCreate() {
        super.onCreate()
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel("rele", "Relé", NotificationManager.IMPORTANCE_LOW))
        val n = Notification.Builder(this, "rele").setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle("Relé de MobileMesh en marcha").setContentText("Relé 7900 · Web 7901 · Guardián 7954").setOngoing(true).build()
        startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        // Sin estos dos, con la pantalla apagada Android duerme la CPU y la wifi.
        getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "rele:cpu").acquire()
        @Suppress("DEPRECATION")
        getSystemService(WifiManager::class.java).createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "rele:wifi").acquire()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (hilo == null) {
            val rele = File(filesDir, "rele.log")
            val guardian = File(filesDir, "guardian.log")
            val dirGuardian = File(filesDir, "guardian").path
            val mesh = File(applicationInfo.nativeLibraryDir, "libmeshguardian.so").path
            // directorio.txt: lista firmada de relés y guardianes que el relé entrega a las apps.
            val directorio = listOf("--directorio", File(filesDir, "directorio.txt").path)
            hilo = hilo { vigilar("relé", listOf(File(applicationInfo.nativeLibraryDir, "libmeshrele.so").path) + ARGS + directorio, rele) }
            hilo { puerto(PUERTO, rele) }
            hilo { puerto(PUERTO_WEB, rele) }
            hilo {
                // La primera vez crea la identidad del guardián (si ya existe, no hace nada).
                runCatching { ProcessBuilder(mesh, dirGuardian, "init").redirectErrorStream(true).redirectOutput(ProcessBuilder.Redirect.appendTo(guardian)).start().waitFor() }
                // Ruta pública para su invitación: la de ruta-publica.txt (p. ej. un nombre de
                // DuckDNS) o mi IP pública según un relé. Sin ella se reintenta cada 30 s.
                var ruta: String? = null
                while (ruta == null) {
                    ruta = File(filesDir, "ruta-publica.txt").takeIf { it.exists() }?.readText()?.trim()?.ifEmpty { null }
                        ?: ipPublica()?.let { "$it:$PUERTO_GUARDIAN" }
                    if (ruta == null) Thread.sleep(30_000)
                }
                vigilar("guardián", listOf(mesh, dirGuardian, "guardian", "0.0.0.0:$PUERTO_GUARDIAN", ruta), guardian)
            }
            hilo { puerto(PUERTO_GUARDIAN, guardian) }
        }
        return START_STICKY
    }

    /** Pide al router (UPnP) que abra el puerto hacia este móvil; se renueva cada 30 min, así sigue
     *  abierto aunque cambie la IP del móvil o de wifi, y caduca solo si el móvil se va. */
    /** Mi IP pública: la que ve un relé universal al conectarme (como STUN). */
    private fun ipPublica(): String? = RELES_QUIEN.firstNotNullOfOrNull { (host, puerto) ->
        runCatching {
            java.net.Socket().use { s ->
                s.connect(java.net.InetSocketAddress(host, puerto), 5000)
                s.soTimeout = 5000
                s.getOutputStream().write("QUIEN\n".toByteArray())
                s.getInputStream().bufferedReader().readLine()?.trim()
            }
        }.getOrNull()?.takeIf { it.matches(Regex("""\d{1,3}(\.\d{1,3}){3}""")) }
    }

    private fun hilo(op: () -> Unit) = Thread(op).apply { isDaemon = true; start() }

    private fun puerto(puerto: Int, log: File) {
        var ultimo = ""
        while (true) {
            val r = runCatching { Upnp.abrir(puerto, 3600) }.getOrElse { "no se pudo abrir el puerto: ${it.message}" }
            if (r != ultimo) log.appendText("$r ${java.util.Date()}\n")
            ultimo = r
            // Si falló (sin wifi, router ocupado), se reintenta al minuto; si no, cada 30 min.
            Thread.sleep(if (r.startsWith("puerto")) 30 * 60 * 1000L else 60 * 1000L)
        }
    }

    /** Mantiene un programa en marcha: si se cierra, lo vuelve a abrir a los 3 s. */
    private fun vigilar(nombre: String, orden: List<String>, log: File) {
        while (true) {
            log.appendText("$nombre inicia ${java.util.Date()}\n")
            val code = runCatching {
                ProcessBuilder(orden).redirectErrorStream(true)
                    .redirectOutput(ProcessBuilder.Redirect.appendTo(log)).start().waitFor()
            }.fold({ "$it" }, { it.message })
            log.appendText("$nombre terminó ($code) ${java.util.Date()}\n")
            Thread.sleep(3000)
        }
    }
}

/** Pantalla mínima: arranca el relé, pide no ahorrar batería y muestra el registro. */
class MainActivity : Activity() {
    @SuppressLint("BatteryLife") // es un servidor: tiene que quedar fuera del ahorro de batería
    override fun onCreate(b: Bundle?) {
        super.onCreate(b)
        startForegroundService(Intent(this, ReleService::class.java))
        if (checkSelfPermission("android.permission.POST_NOTIFICATIONS") != 0) requestPermissions(arrayOf("android.permission.POST_NOTIFICATIONS"), 0)
        if (!getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(packageName)) {
            startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")))
        }
        val ultimas = { f: String -> File(filesDir, f).takeIf { it.exists() }?.readLines()?.takeLast(6)?.joinToString("\n") ?: "" }
        setContentView(TextView(this).apply {
            setPadding(48, 120, 48, 48)
            textSize = 16f
            val invitacion = File(filesDir, "guardian.log").takeIf { it.exists() }?.readLines()?.lastOrNull { it.startsWith("invitación:") } ?: "invitación del guardián: todavía no"
            setTextIsSelectable(true)
            text = "Relé (7900) y guardián (7954) de MobileMesh en marcha.\n\n$invitacion\n\nRelé:\n${ultimas("rele.log")}\n\nGuardián:\n${ultimas("guardian.log")}"
        })
    }
}

/** Abrir un puerto en el router de casa por UPnP (sin contraseña). */
private object Upnp {
    private const val SERVICIO = "urn:schemas-upnp-org:service:WANIPConnection:1"

    /** Devuelve una línea para el registro: «puerto abierto…» o lanza el error. */
    fun abrir(puerto: Int, segundos: Int): String {
        val miIp = NetworkInterface.getByName("wlan0")?.inetAddresses?.toList()?.firstOrNull { it is Inet4Address }?.hostAddress
            ?: error("sin wifi")
        val control = control()
        // Si ya apuntaba a otra IP (este móvil antes, u otro aparato), el router no deja cambiarlo.
        runCatching { soap(control, "DeletePortMapping", "<NewRemoteHost></NewRemoteHost><NewExternalPort>$puerto</NewExternalPort><NewProtocol>TCP</NewProtocol>") }
        soap(control, "AddPortMapping", "<NewRemoteHost></NewRemoteHost><NewExternalPort>$puerto</NewExternalPort>" +
            "<NewProtocol>TCP</NewProtocol><NewInternalPort>$puerto</NewInternalPort><NewInternalClient>$miIp</NewInternalClient>" +
            "<NewEnabled>1</NewEnabled><NewPortMappingDescription>rele mobilemesh</NewPortMappingDescription>" +
            "<NewLeaseDuration>$segundos</NewLeaseDuration>")
        return "puerto $puerto abierto en el router hacia $miIp"
    }

    /** Busca el router en la wifi (SSDP) y la dirección para pedirle cosas. */
    private fun control(): String {
        val pregunta = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\n" +
            "ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n"
        val respuesta = DatagramSocket().use { s ->
            s.soTimeout = 4000
            val b = pregunta.toByteArray()
            s.send(DatagramPacket(b, b.size, InetAddress.getByName("239.255.255.250"), 1900))
            val p = DatagramPacket(ByteArray(2048), 2048)
            s.receive(p)
            String(p.data, 0, p.length)
        }
        val ubicacion = Regex("(?i)location:\\s*(\\S+)").find(respuesta)?.groupValues?.get(1) ?: error("el router no dice dónde está")
        val xml = URL(ubicacion).readText()
        val ruta = Regex("$SERVICIO</serviceType>.*?<controlURL>([^<]+)", RegexOption.DOT_MATCHES_ALL).find(xml)?.groupValues?.get(1)
            ?: error("el router no permite abrir puertos")
        return URL(URL(ubicacion), ruta).toString()
    }

    private fun soap(url: String, accion: String, cuerpo: String): String {
        val c = URL(url).openConnection() as HttpURLConnection
        c.requestMethod = "POST"
        c.doOutput = true
        c.connectTimeout = 8000
        c.readTimeout = 8000
        c.setRequestProperty("Content-Type", "text/xml; charset=\"utf-8\"")
        c.setRequestProperty("SOAPAction", "\"$SERVICIO#$accion\"")
        c.outputStream.use {
            it.write(("<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" " +
                "s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:$accion xmlns:u=\"$SERVICIO\">" +
                "$cuerpo</u:$accion></s:Body></s:Envelope>").toByteArray())
        }
        if (c.responseCode != 200) error("el router respondió ${c.responseCode} a $accion")
        return c.inputStream.bufferedReader().readText()
    }
}
