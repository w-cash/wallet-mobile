# Wcash Mobile exact-fork adapter

## Invariants

- UI baseline: Zingo Mobile `zingo-2.0.23-317` at `bc9b47e0b3ccdd2735e7d381f1c76d58a8628b80`.
- Wcash core: `w-cash/wallet-core` at `58bc22ec63bbe3eddab5f961c137836431589c95`.
- Wcash wallet backend: `w-cash/wolf` at `5b4e29980eb45e84ddab9024f530c923986d7e1e`.
- Consensus patches: `w-cash/wolf` at `9a9c0668784117f116d5b69bdb3a090765092343`.
- Product name: `Wcash Wallet`.
- Production ticker copy: `WEC`; Testnet and Regtest runtime ticker: `TWC`.
- Native identity: `com.wcashwallet.wallet`; beta: `com.wcashwallet.wallet.beta`.
- Wcash Mainnet is unavailable. Only Testnet and feature-gated Regtest exist in the reviewed core.

The upstream React screens, component hierarchy, and navigation remain intact. `scripts/assert-upstream-ui-parity.mjs` pins every byte under `app`, `screens`, `ui`, and `assets`: 450 files must match upstream as one digest, and 24 reviewed branding/protocol files must match individual digests. Source copyright and license files remain in the repository; user-facing About attribution uses the required Wcash Wallet product name.

## First backend slice

`rust/wcash-mobile-adapter` is an additive crate. It calls `zingolib::wcash::{WcashTestnetRuntime, WcashRegtestRuntime}` and never calls the upstream Zcash `LightClient`.

`rust/wcash-mobile-ffi` is the native UniFFI boundary. It retains the upstream `uniffi.zingo` namespace and function names so the Kotlin, Swift, and React contracts stay stable, but it depends only on `wcash-mobile-adapter`. Android and iOS build scripts explicitly build this package. Both native bridges call its Wcash-only `set_wallet_directory` symbol during initialization, so accidentally packaging the upstream Zcash FFI becomes a build/link failure instead of a runtime fallback.

| Existing mobile contract | Adapter method | State |
| --- | --- | --- |
| `init_new` | `create_new` | Generates a 24-word BIP-39 phrase and creates a Wcash account against an attested endpoint. |
| `init_from_seed` | `restore_from_seed` | Restores Testnet or Regtest from phrase and birthday. |
| `init_from_bytes` | `open_wallet_bytes` | Verifies a bounded checksummed Wcash envelope, restores its SQLite database, and opens it on the exact network. |
| `save_wallet_bytes` | `save_wallet_bytes` | Saves version, Wcash network, birthday, seed phrase, and SQLite bytes with SHA-256 integrity. The leading little-endian version remains compatible with the native plain-wallet probe. |
| `read_wallet_recovery_info` | free function of the same name | Reads seed, birthday, and chain without opening SQLite or contacting a server. |
| `validate_wallet_bytes` | free function of the same name | Rejects wrong magic, network, bounds, length, seed, checksum, and trailing bytes. |
| `run_sync` / `pause_sync` / `poll_sync` | `prepare_sync`, `synchronize`, `pause_sync` | Uses the core's bounded cooperative cancellation and returns the existing completion JSON shape. |
| `status_sync` | `sync_status_json` | Reports exact scanned/tip state. The core does not expose live within-batch output counts, so those fields remain zero and percentage is fail-closed at 0 until complete. |
| `get_latest_block_wallet` | `latest_wallet_block_json` | Returns the highest fully scanned height. |
| `get_latest_block_server` | `latest_server_height` | Tries only the two frozen Wcash network identities and returns a tip after the endpoint attests as Testnet or Regtest. A Zcash or otherwise mismatched endpoint fails closed. |
| `info_server` | `server_info_json` | Returns the active Wcash network, branch ID, attested tip, endpoint, and launch activation heights in the existing UI shape. |
| `get_balance` | `balance_json` | Maps Ironwood, transparent, and disabled legacy pools to the existing React shape. |
| `get_spendable_balance_total` | `spendable_balance_json` | Sums immediately spendable Ironwood value. |
| `get_unified_addresses` | `unified_addresses_json` | Returns the canonical Wcash Ironwood Unified Address in the existing list shape. |
| `get_transparent_addresses` | `transparent_addresses_json` | Returns the deterministic Wcash transparent coinbase receiver. |
| `parse_address` | `parse_address_json` | Accepts only canonical Wcash UAs on Testnet or Regtest with the mandatory Ironwood receiver; Zcash and cross-network addresses fail. The existing `orchard` receiver token is retained only as the unchanged React contract. |
| `get_value_transfers` | `confirmed_history_json` | Maps bounded, newest-first confirmed Wcash history; unavailable recipient and memo details are omitted. |
| `send` | `stage_send_json` | Validates exact-network recipients and records a side-effect-free preview; repeated form and confirmation-screen calls may replace an unsigned preview. It returns the ZIP 317 minimum fee and Ironwood pool JSON. |
| `confirm` | `confirm_send_json` | Proves and signs only after confirmation. It broadcasts only when the signed fee exactly matches the reviewed preview, retains the exact signed bytes across a retry, and clears them only after successful broadcast. |

The current core has no pure proposal or fee-quote API. The adapter therefore uses the ZIP 317 minimum for the initial preview and leaves the wallet database unchanged while Zingo Mobile recalculates that preview. After the user confirms, the adapter signs once. If the actual signed fee differs, it fails closed and retains those exact signed bytes; the next unchanged preview exposes the exact fee, and a second confirmation broadcasts the same bytes. A core proposal API is still required to avoid that extra review cycle when the actual fee exceeds the minimum.

## Fail-closed boundary

The native Android and iOS wallet build paths select `wcash-mobile-ffi`; they no longer package the upstream `rust/lib` wallet implementation. Every UDL entry point exists in the Wcash implementation. Supported first-slice calls route to the adapter, deliberately neutral donation calls return an empty address, and every other call rejects with a typed error containing `unsupported Wcash feature`. The upstream `rust/lib/src/lib.rs` remains byte-identical to the audited tag and is retained only as the reference implementation.

The unchanged periodic data coordinator consumes the explicit memo-history capability error as an empty message projection so it does not recursively restart synchronization. The Messages screen and navigation remain present; no native method reports unsupported memo data as successful wallet history.

The shared core still lacks contracts needed for UFVK/watch-only wallets, offline creation, pure send proposals, detailed history recipients and memos, diversified address creation, live sync progress, extended server diagnostics, market price, message analytics, rescan, and the upstream migration/split/Nym features. Wcash Mainnet consensus identity is also not frozen. These surfaces must fail closed; they must never fall through to Zcash.

The local shared-core commit `3719a2006c2a5924e725fad6e1ba4d9a8f6823fb` adds facade-level recipient validation and seedless balance/history reads, but it is not pushed and therefore is not a reproducible Cargo git revision yet. This mobile branch stays on the last remote immutable Wcash core (`58bc22ec...`). Once that core commit is integrated and pushed, mobile should repin and replace its equivalent wolf recipient call with `WcashTestnet::validate_recipient` / `WcashRegtest::validate_recipient`. A facade for wallet-less endpoint attestation is still needed before the adapter can remove its direct wolf dependency completely.

No Wcash donation address or Wcash block explorer is approved. Donation creation, prompts, settings persistence, and automatic address-book insertion are disabled without changing their screen structure. Explorer URLs return empty. Support actions open the verified `https://github.com/w-cash/wallet-mobile/issues` target; no Wcash email address is invented. An approved Wcash support email remains a release blocker if email support is required for store release.

The local build exposes only the reviewed `http://127.0.0.1:48234` Wcash Regtest endpoint. Upstream Zcash endpoints and registry lookup are absent. Add a public Wcash Testnet server only after its endpoint is approved and attests to the frozen Testnet identity.

## Pull-request CI

`.github/workflows/wcash-pr-qa.yaml` is the Wcash fork's automatic pull-request gate. It uses only GitHub-hosted `ubuntu-24.04` and `macos-15` runners and has read-only repository permissions. It checks source parity and native identity, tests both Wcash Rust crates on the host, cross-compiles `wcash-mobile-ffi` for Android ARM64, cross-compiles it for the iOS ARM64 simulator, and generates/type-checks the matching Kotlin and Swift contracts. It contains no release, upload, signing, or deployment step.

The original `.github/workflows/ci.yaml` fan-out is manual-only in this fork. It hard-codes the upstream `*/zingo-mobile` checkout, private `zingo-android-*` / `zingo-ios-build-12` runner labels, and Zcash integration topology, so it cannot be a truthful Wcash PR gate without a broader migration. The new hosted workflow has not run on GitHub until this branch is pushed; a green local YAML/source assertion cannot substitute for its first hosted run.

## Verification

Run from the repository root:

```sh
node scripts/assert-upstream-ui-parity.mjs
yarn jest __tests__/WcashNativeIdentity.unit.test.ts --runInBand
yarn typecheck
```

Run the Rust adapter from `rust` with the reviewed local toolchain:

```sh
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk \
PROTOC=/Users/rustdev/Documents/Codex/2026-09-12/weh/work/toolchains/protoc-36.1-install/bin/protoc \
cargo test -p wcash-mobile-adapter
cargo test -p wcash-mobile-ffi
cargo test -p wcash-mobile-adapter tests::local_regtest_endpoint_attests_and_reports_server_info -- --ignored --exact
cargo test -p wcash-mobile-adapter tests::local_regtest_first_slice_create_sync_save_and_restore -- --ignored --exact
cargo test -p wcash-mobile-ffi tests::local_regtest_native_boundary_runs_the_first_slice -- --ignored --exact
# With WCASH_MOBILE_FUNDED_TEST_SEED and the documented local miner variables:
cargo test -p wcash-mobile-ffi tests::local_regtest_funded_send_mine_sync_and_reopen_through_ffi -- --ignored --exact
```

The full native builds and emulator/device runs require full Xcode plus CocoaPods, or a JDK plus Android SDK/NDK and an emulator. Those toolchains are not present on this host. The host-side UniFFI library and live Regtest boundary are nevertheless executable with the reviewed Rust toolchain.

The disposable funded test keeps its 24-word phrase only in the process environment. It creates a recipient through the mobile FFI, restores and synchronizes the funded sender through that same boundary, previews without changing wallet bytes, confirms and broadcasts, invokes the merged Regtest miner, synchronizes again, verifies the transaction is confirmed in history, then exports and reopens the wallet through FFI and verifies that history again. The phrase is never written to source, test output, miner arguments, or the repository.

On this host, the parity guard passed with `450 + 24` files, all `89` Jest suites (`621` tests and `95` snapshots) passed, TypeScript, ESLint, Prettier, Rustfmt, and Clippy passed, and both Rust crates passed their ordinary tests. The two adapter integration tests, the fresh-wallet native-FFI integration test, and the complete funded send/broadcast/mine/sync/history/reopen FFI test all passed against the live local Wcash Regtest endpoint.
