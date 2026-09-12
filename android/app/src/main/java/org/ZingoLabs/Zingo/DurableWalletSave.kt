package org.ZingoLabs.Zingo

internal fun requireDurableInitialWalletSave(saved: Boolean) {
    check(saved) { "Wcash Wallet initialization could not be saved durably." }
}
