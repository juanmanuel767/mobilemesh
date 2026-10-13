package org.mobilemesh.sdk

import android.app.KeyguardManager
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import uniffi.mobilemesh_core.KeyGuard
import uniffi.mobilemesh_core.MeshException

/**
 * Cifra la identidad con una clave AES que vive en Android Keystore: esa clave no se puede
 * sacar del teléfono, así que copiar el archivo de identidad a otro equipo no sirve de nada.
 * Solo funciona con el teléfono desbloqueado (PIN, patrón o huella).
 */
class KeystoreGuard(private val context: Context) : KeyGuard {
    private val alias = "mobilemesh-identidad"

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(alias, null) as? SecretKey)?.let { return it }
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setUnlockedDeviceRequired(true)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply { init(spec) }.generateKey()
    }

    override fun seal(plain: ByteArray): ByteArray {
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        return c.iv + c.doFinal(plain)
    }

    override fun open(sealed: ByteArray): ByteArray {
        // Con el teléfono bloqueado Keystore no entrega la clave: no es que esté dañada.
        if (context.getSystemService(KeyguardManager::class.java).isDeviceLocked) {
            throw MeshException.Fallo("BLOQUEADO: desbloquea el teléfono para abrir la app")
        }
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, 12))
        return c.doFinal(sealed, 12, sealed.size - 12)
    }
}
