package org.ZingoLabs.Zingo

import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod

internal fun unsupportedWcashMixnetTransport(): Nothing =
    throw UnsupportedOperationException(
        "Wcash Wallet mixnet transport is unsupported by the reviewed Wcash backend.",
    )

/**
 * Keeps the upstream React Native module surface while ensuring a Wcash build
 * cannot load or start the upstream Nym shim.
 */
class NymTransportModule internal constructor(reactContext: ReactApplicationContext?) :
    ReactContextBaseJavaModule(reactContext) {
    override fun getName(): String = "NymTransportModule"

    @ReactMethod
    fun startMixnetTransport(promise: Promise) {
        FfiOutcome.settling(promise, "start_mixnet_transport") {
            unsupportedWcashMixnetTransport()
        }
    }

    @ReactMethod
    fun stopMixnetTransport(promise: Promise) {
        promise.resolve(null)
    }
}
