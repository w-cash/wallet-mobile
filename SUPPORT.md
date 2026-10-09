# Wcash Wallet Mobile support

Wcash Wallet Mobile is a developer preview. The project has no supported
consumer release, store distribution, or guaranteed support response time.
GitHub prerelease packages are engineering candidates.

Use [GitHub issues](https://github.com/w-cash/wallet-mobile/issues) for public,
non-sensitive build failures and reproducible bugs. Search existing issues
before opening one. Use the private route in [`SECURITY.md`](./SECURITY.md) for
security vulnerabilities.

Include these diagnostics when they apply:

- the Android or iOS version and device architecture
- the Wcash Wallet version, build number, and full source revision
- the package filename and its calculated SHA-256 value
- the exact build or test command and sanitized error output
- reproduction steps using a fresh Regtest wallet
- the approximate event time with a time zone

Remove recovery phrases, spending keys, full viewing keys, wallet files,
addresses, transaction identifiers, balances, QR codes, filesystem paths, IP
addresses, access tokens, and signing credentials from public diagnostics.
Inspect screenshots and video frames before upload.

The current Mainnet candidate uses a plaintext wallet service. Do not use the
candidate with material funds. The Android `prodDebug` APK and unsigned iOS
archives are unsupported. The iOS device archive requires developer signing
and provisioning and is not an ordinary iPhone installation package.

Support requests cannot recover a lost recovery phrase, reverse a confirmed
transaction, or remove public blockchain records. Anyone with a recovery
phrase can control its funds. Move funds to a new wallet if a phrase or
spending key becomes exposed.
