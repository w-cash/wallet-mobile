# Wcash Wallet mobile release gate

This repository preserves the Zingo Mobile UI and builds the Android and iOS editions of **Wcash Wallet**.

| Channel | iOS bundle ID | Android application ID | Display name |
| --- | --- | --- | --- |
| Production | `com.wcashwallet.wallet` | `com.wcashwallet.wallet` | `Wcash Wallet` |
| Beta | `com.wcashwallet.wallet.beta` | `com.wcashwallet.wallet.beta` | `Wcash Wallet` |

Publishing is disabled during local Regtest integration. Wcash Mainnet cannot be built until its consensus identity is frozen.

## Required gates

1. Publish and review the shared Wolf/core commits used by the adapter.
2. Replace every absolute local Rust path with immutable Git revisions and regenerate `rust/Cargo.lock`.
3. Verify exactly one `wcash-wallet` package in the lockfile and dependency graph, with no obsolete `5b4e299...` runtime.
4. Pass UI parity, Jest, TypeScript, ESLint, Prettier, Rustfmt, Clippy, and host Rust tests.
5. Pass the funded Regtest create/restore/sync/receive/shield/send/history/restart test through the mobile FFI.
6. Build, install, cold-launch, and exercise the Android APK on the API 34 ARM64 emulator.
7. Pass the GitHub-hosted macOS iOS boundary build and native identity checks.
8. Repeat the funded lifecycle on an approved, attested public Wcash Testnet endpoint.
9. Supply approved platform signing identities, store records, privacy metadata, support contact, and release metadata.

## Build boundary

Every Android and iOS workflow must build `--package wcash-mobile-ffi`. The upstream `rust/lib` implementation is an audited reference and must never be packaged in a Wcash-branded app. Generated Kotlin/Swift bindings and packaged libraries must expose `set_wallet_directory`; native bridges must call it before wallet initialization.

`.github/workflows/android-release.yaml` creates review artifacts only. It has no tag trigger, signing, publishing, deployment, or GitHub Release action. Its dependency, symbol, package identity, scheme, and display-name checks fail before APK collection.

## Local checks

From the repository root:

```sh
node scripts/assert-upstream-ui-parity.mjs
yarn test --runInBand
yarn typecheck
yarn lint:check
yarn prettier:check
```

From `rust` with the reviewed local toolchain:

```sh
cargo test --locked -p wcash-mobile-adapter -p wcash-mobile-ffi
cargo clippy --locked -p wcash-mobile-adapter -p wcash-mobile-ffi --all-targets -- -D warnings
```

The candidate and pull-request workflows deliberately reject this local validation state until the unpublished shared dependencies become immutable remote revisions.
