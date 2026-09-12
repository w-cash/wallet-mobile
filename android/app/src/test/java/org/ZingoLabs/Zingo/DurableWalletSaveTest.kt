package org.ZingoLabs.Zingo

import org.junit.Assert.assertThrows
import org.junit.Test

class DurableWalletSaveTest {
    @Test
    fun successfulInitialSaveReturnsNormally() {
        requireDurableInitialWalletSave(true)
    }

    @Test
    fun failedInitialSaveRejectsWalletInitialization() {
        assertThrows(IllegalStateException::class.java) {
            requireDurableInitialWalletSave(false)
        }
    }
}
