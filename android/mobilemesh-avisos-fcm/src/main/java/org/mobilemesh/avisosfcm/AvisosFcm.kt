package org.mobilemesh.avisosfcm

import android.content.Context
import com.google.firebase.FirebaseApp
import com.google.firebase.FirebaseOptions
import com.google.firebase.messaging.FirebaseMessaging
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import org.mobilemesh.sdk.Avisos
import org.mobilemesh.sdk.MeshHolder

/**
 * Avisos al instante por Firebase (opcional). Google solo transporta «tienes algo»: el mensaje sigue
 * cifrado en el buzón de tu relé y la app lo recoge al despertar. Firebase no ve contenido, ni
 * remitente, ni a quién escribes; sí sabe que este teléfono recibió un aviso y cuándo.
 *
 * ```
 * AvisosFcm.activar(context, FirebaseOptions.Builder()
 *     .setProjectId("mi-proyecto").setApplicationId("1:123:android:abc")
 *     .setApiKey("AIza…").setGcmSenderId("123").build())
 * ```
 * El relé tiene que arrancarse con `--avisos tools/avisar-fcm.py` (ver docs/sdk.md).
 */
object AvisosFcm {
    /** Activa los avisos. Llamar después de `MobileMesh.iniciar` y con un relé configurado. */
    fun activar(context: Context, opciones: FirebaseOptions) {
        val app = context.applicationContext
        if (FirebaseApp.getApps(app).isEmpty()) FirebaseApp.initializeApp(app, opciones)
        FirebaseMessaging.getInstance().token.addOnSuccessListener { registrar(app, it) }
    }

    /** Deja de recibir avisos por Firebase (quita la ficha del relé). */
    fun desactivar(context: Context) {
        val app = context.applicationContext
        Thread { runCatching { MeshHolder.open(app)?.setPushToken(null) } }.start()
        FirebaseMessaging.getInstance().deleteToken()
    }

    internal fun registrar(c: Context, ficha: String) {
        // ponytail: si falla (sin red) se reintenta en el próximo arranque o cambio de ficha.
        Thread { runCatching { MeshHolder.open(c)?.setPushToken(ficha) } }.start()
    }
}

class AvisosFcmService : FirebaseMessagingService() {
    override fun onNewToken(token: String) = AvisosFcm.registrar(applicationContext, token)

    override fun onMessageReceived(m: RemoteMessage) {
        if (m.data["mm"] != "1") return
        // Android da unos segundos a la app (aunque esté congelada): recoger el buzón y avisar.
        val n = runCatching { MeshHolder.open(applicationContext)?.checkMailbox()?.toInt() ?: 0 }.getOrDefault(0)
        if (n > 0) Avisos.mensajeNuevo(applicationContext, n)
    }
}
