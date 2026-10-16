package org.mobilemesh.reactnative

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/** Recibe «<id de la app>://pago» (regreso de una pasarela de pago) y trae la app al frente. */
class VolverActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        packageManager.getLaunchIntentForPackage(packageName)?.let {
            startActivity(it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT))
        }
        finish()
    }
}
