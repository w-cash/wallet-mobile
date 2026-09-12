# Wcash Wallet release readiness

This repository builds the iOS and Android editions of **Wcash Wallet** from
the same upstream React Native screens.

| Channel | iOS bundle ID | Android application ID | Display name |
| --- | --- | --- | --- |
| Production | `com.wcashwallet.wallet` | `com.wcashwallet.wallet` | `Wcash Wallet` |
| Beta | `com.wcashwallet.wallet.beta` | `com.wcashwallet.wallet.beta` | `Wcash Wallet` |

Production and beta use the Wcash-only `wcash-mobile-ffi` native boundary.
Kotlin and Swift keep the internal `uniffi.zingo` namespace for source
compatibility with the upstream application. Internal Xcode targets, React
module names, and source paths also retain their upstream names.

## Current release gate

Publishing is disabled while this exact fork is under local Regtest review.
Only Wcash Testnet and Regtest identities exist in the reviewed core; Wcash
Mainnet is unavailable until its consensus identity is frozen. Do not create a
store archive, public APK/AAB, release tag, or GitHub Release from this branch.

A release candidate requires all of the following:

1. `scripts/assert-upstream-ui-parity.mjs` passes.
2. The Wcash pull-request QA workflow passes on GitHub-hosted Android and iOS
   builders.
3. A funded Regtest device run proves create, backup, restore, sync, receive,
   send, confirmation, mining, history, and restart.
4. A funded public Testnet device run proves the same lifecycle against an
   approved, attested Wcash endpoint.
5. Every unsupported native method either has a reviewed Wcash implementation
   or fails closed with an `unsupported Wcash feature` error.
6. Approved Apple and Android signing identities, store records, privacy text,
   support contact, and release metadata are available.

## Native build boundary

- Android build scripts generate Kotlin bindings from
  `rust/wcash-mobile-ffi/src/zingo.udl` and package only
  `wcash-mobile-ffi`'s `libzingo.so`.
- The iOS build script generates Swift bindings from the same UDL and packages
  only `wcash-mobile-ffi`'s `libzingo.a` in the wallet XCFramework.
- Both native bridges call the Wcash-only `set_wallet_directory` symbol. A
  mistakenly packaged upstream wallet library therefore fails during native
  compilation or linking.

The automatic `.github/workflows/wcash-pr-qa.yaml` workflow has read-only
repository permissions and performs no signing, upload, release, or deployment.
The inherited upstream CI fan-out remains manual-only because its reusable jobs
refer to upstream repository paths, private runners, and non-Wcash fixtures.

## Local verification

Run JavaScript and parity checks from the repository root:

```sh
node scripts/assert-upstream-ui-parity.mjs
yarn test --runInBand
yarn typecheck
yarn lint:check
yarn prettier:check
```

Run the Wcash native boundary checks from `rust`:

```sh
cargo test --locked -p wcash-mobile-adapter -p wcash-mobile-ffi
cargo clippy --locked -p wcash-mobile-adapter -p wcash-mobile-ffi --all-targets -- -D warnings
```

Full application builds require Xcode and CocoaPods for iOS or a JDK plus the
Android SDK/NDK for Android. Store archives additionally require the approved
platform signing identities and store credentials.
