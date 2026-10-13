package org.mobilemesh.sdk

import android.app.Activity
import android.app.AlertDialog
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings

/**
 * Pide al usuario, uno por uno, los permisos para que MobileMesh funcione al 100 % (recibir con la
 * app cerrada y la pantalla apagada): avisos, batería sin restricciones e inicio automático del
 * fabricante. Llamar en `onResume` de la pantalla principal: solo pregunta lo que falte, una vez por
 * cada vez que se abre la app.
 */
object Permisos {
    private val preguntadoEnEstaSesion = mutableSetOf<String>()

    /** Pantallas de «inicio automático» de cada fabricante (no hay forma estándar en Android). */
    private val inicioAutomatico = listOf(
        "com.transsion.phonemaster" to "com.cyin.himgr.autostart.AutoStartActivity", // TECNO, Infinix, itel
        "com.miui.securitycenter" to "com.miui.permcenter.autostart.AutoStartManagementActivity", // Xiaomi
        "com.huawei.systemmanager" to "com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity",
        "com.coloros.safecenter" to "com.coloros.safecenter.permission.startup.StartupAppListActivity", // Oppo, Realme
        "com.vivo.permissionmanager" to "com.vivo.permissionmanager.activity.BgStartUpManagerActivity",
        "com.zte.powersavemode" to "com.zte.powersavemode.appspowersave.AppManagerActivityMFV", // ZTE, nubia
        "com.samsung.android.lool" to "com.samsung.android.sm.battery.ui.BatteryActivity",
    )

    /** ¿Falta algo que se pueda comprobar? (El inicio automático no se puede comprobar.) */
    fun faltan(c: Context): Boolean = !avisos(c) || !bateria(c) || !alarmas(c)

    fun pedir(a: Activity) {
        when {
            !avisos(a) && preguntar("avisos") ->
                a.requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 7801)
            !bateria(a) && preguntar("bateria") -> explicar(
                a,
                "Recibir mensajes con el teléfono bloqueado",
                "Para que te lleguen los mensajes aunque la app esté cerrada, permite que funcione sin " +
                    "restricciones de batería. En la siguiente ventana pulsa «Permitir».",
            ) { a.startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${a.packageName}"))) }
            !alarmas(a) && preguntar("alarmas") -> explicar(
                a,
                "Mensajes más rápidos",
                "Para mirar si tienes mensajes cada pocos minutos aunque la pantalla esté apagada, " +
                    "activa «Alarmas y recordatorios» para «${nombreApp(a)}» en la siguiente pantalla.",
            ) { a.startActivity(Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, Uri.parse("package:${a.packageName}"))) }
            !yaMostrado(a, "inicio") && preguntar("inicio") -> explicar(
                a,
                "Inicio automático",
                "Tu teléfono cierra las apps para ahorrar batería. Busca «${nombreApp(a)}» en la " +
                    "siguiente pantalla y actívale el inicio automático (o «sin restricciones») para " +
                    "que reciba mensajes al instante.",
            ) {
                marcarMostrado(a, "inicio")
                abrirInicioAutomatico(a)
            }
        }
    }

    private fun avisos(c: Context) = Build.VERSION.SDK_INT < 33 ||
        c.checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    private fun alarmas(c: Context) = Build.VERSION.SDK_INT < 31 || c.getSystemService(android.app.AlarmManager::class.java).canScheduleExactAlarms()

    private fun bateria(c: Context) = c.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(c.packageName)

    private fun preguntar(clave: String) = preguntadoEnEstaSesion.add(clave)

    private fun prefs(c: Context) = c.getSharedPreferences("mobilemesh", Context.MODE_PRIVATE)
    private fun yaMostrado(c: Context, clave: String) = prefs(c).getBoolean("permiso_$clave", false)
    private fun marcarMostrado(c: Context, clave: String) = prefs(c).edit().putBoolean("permiso_$clave", true).apply()

    /** Nombre de la app que usa MobileMesh (el que ve el usuario). */
    internal fun nombreApp(c: Context) = c.applicationInfo.loadLabel(c.packageManager).toString()

    private fun explicar(a: Activity, titulo: String, texto: String, alAceptar: () -> Unit) {
        if (a.isFinishing) return
        AlertDialog.Builder(a)
            .setTitle(titulo)
            .setMessage(texto)
            .setPositiveButton("Continuar") { _, _ -> alAceptar() }
            .setNegativeButton("Ahora no", null)
            .show()
    }

    private fun abrirInicioAutomatico(a: Activity) {
        for ((pkg, cls) in inicioAutomatico) {
            val i = Intent().setComponent(ComponentName(pkg, cls)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            if (runCatching { a.startActivity(i) }.isSuccess) return
        }
        // Fabricante desconocido: los ajustes de la app (batería, segundo plano).
        a.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${a.packageName}")))
    }
}
