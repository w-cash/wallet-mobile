# Wcash Mobile exact Zingo fork

## Source invariants

- Upstream UI baseline: Zingo Mobile `zingo-2.0.23-317`, commit `bc9b47e0b3ccdd2735e7d381f1c76d58a8628b80`.
- Product label: `Wcash Wallet`.
- Native identity: `com.wcashwallet.wallet`; beta: `com.wcashwallet.wallet.beta`.
- Production ticker copy: `WEC`; Testnet and Regtest runtime ticker: `TWC`.
- Wcash Mainnet is unavailable. The reviewed backend exposes Testnet and feature-gated Regtest only.

The React screens, styles, component hierarchy, and navigation remain the upstream implementation. `scripts/assert-upstream-ui-parity.mjs` verifies 447 UI files as one byte-identical upstream digest and 27 individually reviewed exceptions for branding, protocol copy, and availability gates. ZingoLabs/Zingo copyright and MIT attribution remain accurate in every About translation. Internal Zingo module, target, type, and UniFFI names remain where source compatibility requires them.

The upstream `rust/lib` implementation remains byte-identical. No Wcash native build or workflow packages it.

## Reproducibility gate

The corrective adapter currently uses local Wolf commit `8f61c6a36e5eca756d93ba02ec5567fb03c8b312`, including staged transactions, retry-safe proposal cancellation, seed verification, and Android sandbox-aware path validation. That commit is not published. The remote wallet-core pin is still `58bc22ec63bbe3eddab5f961c137836431589c95`, with its transitive Wolf runtime replaced locally during validation so Cargo resolves exactly one `wcash-wallet` package.

This state is intentionally unreleasable. The pull-request and candidate workflows reject absolute Rust paths, require exactly one `wcash-wallet` lockfile entry, and reject the obsolete Wolf revision `5b4e29980eb45e84ddab9024f530c923986d7e1e`. Before review, replace the local direct dependency and patch with an immutable published wallet-core revision that pins the reviewed Wolf commit, regenerate `Cargo.lock`, and rerun every native gate.

## Wcash-only native boundary

`rust/wcash-mobile-adapter` contains the Wcash runtime adapter. `rust/wcash-mobile-ffi` preserves the upstream `uniffi.zingo` contract while depending only on that adapter. Android and iOS build scripts and reusable workflows select `--package wcash-mobile-ffi`. Both platform bridges call the Wcash-only `set_wallet_directory` symbol during initialization, so an upstream Zcash library fails during packaging/linking instead of becoming a runtime fallback.

The implemented lifecycle is:

| Existing mobile call | Wcash behavior |
| --- | --- |
| `init_new`, `init_from_seed` | Create or restore a 24-word Wcash Testnet/Regtest wallet against an attested endpoint. |
| `init_from_bytes` | Validate checksum, network, SQLite structure, birthday, and phrase ownership in a private temporary database, then install with rollback of the main DB, WAL, and SHM on any failure. |
| `save_wallet_bytes` | Checkpoint SQLite and export a bounded, checksummed envelope containing network, birthday, phrase, and database. |
| `run_sync`, `poll_sync`, `status_sync` | Run cancellable Wcash sync and project a truthful `Scanned` range from birthday to the fully scanned height after completion so the unchanged Sync Report leaves its waiting state. |
| balance/address/history calls | Return Ironwood, transparent coinbase, canonical Wcash UA, and confirmed/pending history in the upstream JSON shapes. |
| `send` | Validate exact-network recipients and retain an exact staged proposal. Repeated identical previews return the same exact fee. Replacing a preview cancels only that proposal's locks. |
| `shield` | Stage mature transparent coinbase shielding and return its exact value and fee through the existing confirmation flow. |
| `confirm` | Reverify phrase ownership, calculate and durably store the exact consented transaction, then broadcast those exact bytes. |

Mobile metadata is stored in the wallet SQLite database before broadcast. It keeps outgoing sends and shields visible after immediate restart, including core pending rows created before mobile metadata existed. Once mined, metadata preserves the truthful `shield` classification and transparent-to-Ironwood pool projection even though the current core history reports that transaction as a fee-only outgoing transfer.

If the node accepts a transaction and a later metadata/checkpoint operation fails, `confirm` still returns the accepted txid and keeps the in-memory conflict blocker. It never reports an ordinary send failure after network acceptance.

The reviewed core cannot safely reconstruct a `CalculatedTransaction` after a crash between durable signing and broadcast because its stored signed record lacks the exact tip/anchor pair. Such a transaction remains visible as `calculated`, conflicting proposals remain locked, and mobile fails closed. A shared-core reconstruction/rebroadcast API is required before automated post-crash rebroadcast can be enabled; raw-byte broadcast is forbidden.

## Platform storage

Android uses `context.noBackupFilesDir/wcash-wallet`, creates the leaf directory, and applies `OsConstants.S_IRWXU` before calling the native boundary. Wolf's Android-only validator trusts the SELinux app sandbox boundary while retaining immediate-parent mode, ownership, regular-file, symlink, hard-link, and inode checks. macOS/Linux ancestor traversal remains unchanged.

iOS stores SQLite in `Application Support/WcashWallet`. The directory and `wcash-wallet.sqlite`, WAL, and SHM receive class-C file protection and backup exclusion. Protection functions throw; initialization fails closed and save propagates any protection failure.

Android also retains `allowBackup=false`.

## Visible unavailable features

Ordinary UA receive, mining receive, sync, balance, history, send, and shielding remain available. The existing layout gates UFVK/watch-only import, Rescan, diversified-address creation, transaction removal, and Nym/mixnet because the reviewed Wcash backend does not implement their exact semantics. Their native entry points fail closed and never call the upstream Zcash FFI.

No Wcash donation address, block explorer, or support email is approved. Donation state is forced off while its existing layout remains; address helpers return no destination. Explorer actions are unavailable. Support opens `https://github.com/w-cash/wallet-mobile/issues` and never uses `support@zingolabs.org`.

## Verification evidence

Local source and host checks on 2026-09-12:

- UI parity: 447 byte-identical upstream files plus 27 reviewed exceptions.
- Jest: 89 suites, 621 tests, and 95 snapshots passed.
- TypeScript, ESLint, Prettier, Rustfmt, and strict Clippy passed.
- Rust host tests: 13 adapter tests and 3 FFI tests passed; network tests are separately ignored by default.
- Live Regtest: endpoint attestation, create, restore, rejected-import preservation, sync, balance, receive, history, export, and reopen passed against `127.0.0.1:48234`.
- Funded FFI: shield preview/confirm/broadcast/restart/mine/sync/history and send preview/confirm/broadcast/restart/mine/sync/history passed. Confirmation blocks were height 430 (`2f21a01c9f6e742bda229893117cd446bef6d80cae48074fd00bf1c929bb20b3`) and height 431 (`392df2b87dace89117b38afc30aab5d8849f89dc32d1da8e83d30d6c37143cb3`). The disposable phrase stayed in the process environment and was not logged or committed.
- Android ARM64: native build, APK assembly, install, package clear, cold launch, Regtest wallet creation, and exact upstream Receive UI passed on the API 34 emulator. The pre-sync-status-fix APK SHA-256 was `028af2da99413903b2c441d2573756d8fc3eb88e4303f0fe56b1549f6ce2d769`; a native rebuild is required to include the completed Sync Report projection.

Full Xcode is not available on this host, so the iOS Rust target and app archive still require the GitHub-hosted `macos-15` boundary job after immutable dependency pins are published. Publishing, signing, deployment, and store release remain disabled.
