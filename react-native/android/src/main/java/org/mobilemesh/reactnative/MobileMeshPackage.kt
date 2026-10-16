package org.mobilemesh.reactnative

import com.facebook.react.ReactPackage
import com.facebook.react.bridge.ReactApplicationContext

class MobileMeshPackage : ReactPackage {
    override fun createNativeModules(ctx: ReactApplicationContext) = listOf(MobileMeshModule(ctx))

    override fun createViewManagers(ctx: ReactApplicationContext) = emptyList<Nothing>()
}
