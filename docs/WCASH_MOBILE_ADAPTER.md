# Wcash Mobile exact Zingo fork

## Source invariants

- Frozen release reference: Zingo Mobile `zingo-2.0.23-317`, commit `bc9b47e0b3ccdd2735e7d381f1c76d58a8628b80`.
- Reviewed integration and UI base: upstream `origin/dev`, commit `0d54fd5b3b32603465ac47fe6f8c187e430ffe26` (the frozen release plus five upstream commits).
- Product label: `Wcash Wallet`.
- Native identity: `com.wcashwallet.wallet`; beta: `com.wcashwallet.wallet.beta`.
- Production ticker copy: `WEC`; Testnet and Regtest runtime ticker: `TWC`.
- Wcash Mainnet is unavailable. The reviewed backend exposes Testnet and feature-gated Regtest only.

The React screens, styles, component hierarchy, and navigation remain the upstream implementation. Against the reviewed `0d54fd5b` base, `scripts/assert-upstream-ui-parity.mjs` verifies 452 UI files as one byte-identical upstream digest and 22 individually reviewed exceptions for branding, Wcash protocol copy, and product assets. The visible About product notice uses Wcash Wallet; upstream ZingoLabs/Zingo copyright and MIT attribution remain intact in the repository's license and third-party notices. Internal Zingo module, target, type, and UniFFI names remain where source compatibility requires them.

The upstream `rust/lib` implementation remains byte-identical and is excluded from the active Wcash Cargo workspace. No Wcash native build or workflow compiles or packages it.

## Reproducibility gate

The manifest and lockfile pin wallet-core commit `5f4e0b45bad15facb5f84ac6217cb5287101bf96` and Wolf commit `f2486f65a6508771eafc634223344499ac23c5a5` through their public GitHub URLs. Cargo resolves exactly one remote-source `wcash-wallet` package and the final Wcash FFI graph contains no Nym or mixnet dependency. Wolf supplies staged transactions, retry-safe proposal cancellation, seed verification, confirmed-history summaries, and mobile sandbox-aware path validation.

At the time of this local verification, clean shallow fetches of those two revisions still returned `not our ref`; the objects were available only in the reviewed local repositories and existing Cargo cache. The pull-request and candidate workflows therefore fail closed before producing an artifact until both exact revisions are published. They also reject absolute Rust paths, require exactly one `wcash-wallet` lockfile entry, and reject the obsolete Wolf revision `5b4e29980eb45e84ddab9024f530c923986d7e1e`. After publication, rerun the locked build with an isolated Cargo home to prove clean reproducibility.

## Wcash-only native boundary

`rust/wcash-mobile-adapter` contains the Wcash runtime adapter. `rust/wcash-mobile-ffi` preserves the upstream `uniffi.zingo` contract while depending only on that adapter. Android and iOS build scripts and the manual candidate workflow select `--package wcash-mobile-ffi`; inherited app-assembly reusable workflows are disabled. Both platform bridges call the Wcash-only `set_wallet_directory` symbol during initialization, so an upstream Zcash library fails during packaging/linking instead of becoming a runtime fallback.

The implemented lifecycle is:

| Existing mobile call                   | Wcash behavior                                                                                                                                                                        |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `init_new`, `init_from_seed`           | Create or restore a 24-word Wcash Testnet/Regtest wallet against an attested endpoint.                                                                                                |
| `init_from_bytes`                      | Validate checksum, network, SQLite structure, birthday, and phrase ownership in a private temporary database, then install with rollback of the main DB, WAL, and SHM on any failure. |
| `save_wallet_bytes`                    | Checkpoint SQLite and export a bounded, checksummed envelope containing network, birthday, phrase, and database.                                                                      |
| `run_sync`, `poll_sync`, `status_sync` | Run cancellable Wcash sync and project a truthful `Scanned` range from birthday to the fully scanned height after completion so the unchanged Sync Report leaves its waiting state.   |
| balance/address/history calls          | Return Ironwood, transparent coinbase, canonical Wcash UA, and confirmed/pending history in the upstream JSON shapes.                                                                 |
| `send`                                 | Validate exact-network recipients and retain an exact staged proposal. Repeated identical previews return the same exact fee. Replacing a preview cancels only that proposal's locks. |
| `shield`                               | Stage mature transparent coinbase shielding and return its exact value and fee through the existing confirmation flow.                                                                |
| `confirm`                              | Reverify phrase ownership, calculate and durably store the exact consented transaction, then broadcast those exact bytes.                                                             |

Mobile metadata is stored in the wallet SQLite database before broadcast. It keeps outgoing sends and shields visible after immediate restart, including core pending rows created before mobile metadata existed. The core's confirmed summary supplies an exact display value when compact-chain data can prove one; an unavailable value remains visible as `unknown` with zero as the upstream JSON shape's explicit sentinel. Mobile metadata upgrades the app's own transactions with their durable send or shield classification and exact value.

If the node accepts a transaction and a later metadata/checkpoint operation fails, `confirm` still returns the accepted txid and keeps the in-memory conflict blocker. It never reports an ordinary send failure after network acceptance.

The reviewed core cannot safely reconstruct a `CalculatedTransaction` after a crash between durable signing and broadcast because its stored signed record lacks the exact tip/anchor pair. Such a transaction remains visible as `calculated`, conflicting proposals remain locked, and mobile fails closed. A shared-core reconstruction/rebroadcast API is required before automated post-crash rebroadcast can be enabled; raw-byte broadcast is forbidden.

## Platform storage

Android uses `context.noBackupFilesDir/wcash-wallet`, creates the leaf directory, and applies `OsConstants.S_IRWXU` before calling the native boundary. Wolf's Android-only validator trusts the SELinux app sandbox boundary while retaining immediate-parent mode, ownership, regular-file, symlink, hard-link, and inode checks. macOS/Linux ancestor traversal remains unchanged.

iOS stores SQLite in `Application Support/WcashWallet`. The directory and `wcash-wallet.sqlite`, WAL, and SHM receive class-C file protection and backup exclusion. Protection functions throw; initialization fails closed and save propagates any protection failure.

Android also retains `allowBackup=false`.

## Visible unavailable features

Ordinary UA receive, mining receive, sync, balance, history, send, and shielding remain available. The existing UFVK/watch-only, Rescan, diversified-address, transaction-removal, and Nym/mixnet controls retain their upstream layout and behavior. Unsupported native entry points fail closed with explicit Wcash errors and never call the upstream Zcash FFI. Disabling the unavailable mixnet is idempotent and returns its canonical `off` state, which keeps clearnet sending available. Wcash build scripts omit the upstream Nym shim.

No Wcash donation address, block explorer, or support email is approved. The upstream donation control remains visible; address helpers return no destination, so no donation transaction can be created without an approved Wcash address. Explorer actions are unavailable. Support opens `https://github.com/w-cash/wallet-mobile/issues` and never uses `support@zingolabs.org`.

## Verification evidence

Local source and host checks completed on 2026-09-13:

- UI parity: 452 byte-identical upstream files plus 22 reviewed exceptions.
- Jest: 89 suites, 622 tests, and 95 snapshots passed.
- TypeScript, ESLint, Prettier, Rustfmt, and strict Clippy for the Wcash adapter/FFI boundary passed.
- Rust host tests: 15 adapter tests and 5 FFI tests passed; two network tests are ignored by default.
- Live Regtest: endpoint attestation, create, restore, rejected-import preservation, sync, balance, receive, history, export, and reopen passed against `127.0.0.1:48234`.
- Funded FFI: shield preview/confirm/broadcast/restart/mine/sync/history and send preview/confirm/broadcast/restart/mine/sync/history passed. Confirmation blocks were height 430 (`2f21a01c9f6e742bda229893117cd446bef6d80cae48074fd00bf1c929bb20b3`) and height 431 (`392df2b87dace89117b38afc30aab5d8849f89dc32d1da8e83d30d6c37143cb3`). The disposable phrase stayed in the process environment and was not logged or committed.
- Final installed-app receive proof: a staged 100,000-zat transfer with exact 10,000-zat fee was broadcast as `df6070f45f64dd1837dc7d6c6d96be799d745b9e64c6b263854dcb109ddbabea` and confirmed in Wcash block 432 (`b5e4a53d1b2745924a8392199884c7420024850b92206344bb44d99695f94e54`). The installed wallet showed `TWC 0.001`, the Ironwood transaction detail, and `Fully Synced` at server height 432.
- Android ARM64: `node rust/android/build_android_local.mjs arm64 --regtest-qa-apk` performs a clean Wcash native build, UniFFI binding generation, 37 JVM unit tests, ARM64 split assembly, and package gates in one command. The API 34 emulator installed and cold-launched the build in 577 ms. The ARM64-only APK has version code 30317, package `com.wcashwallet.wallet`, SHA-256 `5d096a235fb974d9616690a59ced31dd7271677ab093ed24d307fc19a225af9b`, and one Wcash wallet library at `lib/arm64-v8a/libuniffi_zingo.so`. The package contains the full `5f4e0b45bad15facb5f84ac6217cb5287101bf96` core identity and omits the old core identity, `libzingo_nym_proxy_ffi.so`, and generated Nym bindings. The funded History and Send screens opened with the mixnet control off and the Send control active. Sanitized evidence and the APK are in `../../../outputs/wcash-mobile-android-final`, relative to this repository.
- iOS: Xcode 26.6 built the Wcash XCFramework and an ad-hoc-signed Release app for iPhone 17/iOS 26.5. The installed app created a wallet, synced through height 440, displayed a `wuregtest` receiver, broadcast a self-send with all biometric gates disabled, showed pending History, and changed the row to Sent after one paired block. The create/restore/sync/receive/history XCTest and funded self-send/reopen XCTest each passed. Screen evidence is in `../../../outputs/wcash-mobile-ios-final`, relative to this repository.
- The exact npm `react-native-keychain@10.0.0` package receives the upstream `af7612e251d744891d22a7ef935d95052f1328c8` TurboModule patch through `patch-package`. Its published JavaScript and type files remain present. The signed simulator build exercises the patched keychain under React Native's New Architecture.

The GitHub-hosted `macos-26` job uses Xcode 26.6 for the iOS XCFramework, XCTest suite, and compile-only unsigned ARM64 packages after the immutable dependency objects are published. The protected production workflow stops before signing or store delivery.
