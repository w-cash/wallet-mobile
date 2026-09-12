package org.ZingoLabs.Zingo

import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class NymTransportHandleTest {
    @Test
    fun wcashBuildFailsClosedWithoutTheUpstreamMixnetShim() {
        val failure = assertThrows(UnsupportedOperationException::class.java) {
            unsupportedWcashMixnetTransport()
        }
        assertTrue(failure.message.orEmpty().contains("unsupported by the reviewed Wcash backend"))
    }
}
