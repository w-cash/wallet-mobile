# Wcash Wallet mobile build and release gate

This repository retains the upstream Zingo Mobile interface. The reviewed candidate profile uses Wcash Local Regtest. The visible product name is `Wcash Wallet`.

| Channel    | iOS bundle ID                 | Android application ID        | Display name   |
| ---------- | ----------------------------- | ----------------------------- | -------------- |
| Production | `com.wcashwallet.wallet`      | `com.wcashwallet.wallet`      | `Wcash Wallet` |
| Beta       | `com.wcashwallet.wallet.beta` | `com.wcashwallet.wallet.beta` | `Wcash Wallet` |

Wcash Mainnet packaging remains disabled until the consensus identity, addresses, transaction signing domain, public endpoint, and release policy are approved.

## Pull request checks

`.github/workflows/wcash-pr-qa.yaml` runs these independent jobs:

1. The source job checks the upstream UI parity, JavaScript tests, TypeScript, ESLint, Prettier, Rustfmt, Clippy, and the Wcash adapter and FFI tests.
2. The Android job builds the ARM64 Wcash FFI, runs the prodDebug JVM tests and lint, and assembles a complete prodDebug APK with its JavaScript bundle. It checks the package ID, display name, ABI, native library count, signing envelope, and absence of the legacy Nym native library.
3. The iOS job uses the GitHub `macos-26` ARM64 image, Xcode 26.6, Ruby 3.3.12, Bundler 2.3.18, and Rust 1.91.0. It builds the Wcash XCFramework, installs the locked CocoaPods graph, runs the static XCTest suite on the iPhone 17/iOS 26.5 simulator, and links complete unsigned ARM64 Release apps for the simulator and a physical-device target. It checks the bundle ID, display name, JavaScript bundle, architecture, signing state, and absence of the legacy Nym native library.

The pull request APK and both iOS ZIPs remain available for three days. The device ZIP proves the app links for `iphoneos`; installation on a phone still requires Apple signing and provisioning.

## Unsigned candidates

Run **Wcash mobile unsigned candidate** from GitHub Actions on the reviewed commit or release tag. The workflow records `regtest` in the filename and manifest and verifies the reviewed Local Regtest profile. Enable a Testnet candidate only after the repository contains an approved Testnet server profile and its lifecycle test passes.

The workflow produces one aggregate artifact for 14 days:

```text
Wcash-Wallet-regtest-<version>-<build>-<sha>-android-arm64-prodDebug.apk
Wcash-Wallet-regtest-<version>-<build>-<sha>-ios-arm64-simulator-unsigned.zip
Wcash-Wallet-regtest-<version>-<build>-<sha>-ios-arm64-device-compile-unsigned.zip
MANIFEST.json
SHA256SUMS
```

The manifest records the full source revision, network, version, build number, compiler versions, platform, signing state, file size, and SHA-256 digest. The workflow checks every digest before upload.

## Immutable inputs

`scripts/ci/verify-wcash-dependencies.mjs` enforces the approved repository and full 40-character revision for every direct Git dependency. It checks all Git sources in `rust/Cargo.lock`, verifies one `wcash-wallet` package, and fetches each locked revision in CI before compilation.

The repository toolchain and lockfiles control the build inputs:

- Rust 1.91.0
- Node.js 22.18.0 and Yarn 1.22.22
- Java 17, Android NDK 28.2.13676358, and cargo-ndk 4.0.1
- Ruby 3.3.12, Bundler 2.3.18, and CocoaPods from `Gemfile.lock`
- Xcode 26.6 on `macos-26`

The workflows pin each external action to a commit from these reviewed releases:

| Action                          | Release |
| ------------------------------- | ------- |
| `actions/checkout`              | v7      |
| `actions/setup-node`            | v4      |
| `actions/setup-java`            | v6      |
| `actions/upload-artifact`       | v4      |
| `actions/download-artifact`     | v5      |
| `android-actions/setup-android` | v3      |
| `gradle/actions/setup-gradle`   | v6      |
| `dtolnay/rust-toolchain`        | 1.91.0  |
| `Swatinem/rust-cache`           | v2      |
| `maxim-lobanov/setup-xcode`     | v1      |
| `ruby/setup-ruby`               | v1      |

## Production signing

`.github/workflows/mobile-production-signing.yaml` uses the protected `wcash-mobile-production` GitHub environment. Configure required reviewers on that environment before adding credentials. The current gate validates an exact Wcash release tag and an explicit confirmation phrase, then stops before signing.

Keep Android prodRelease and AAB work in this protected workflow. Keep Apple distribution signing and App Store packaging in this protected workflow. Candidate and pull request jobs use prodDebug Android signing and signing-disabled iOS simulator builds. No workflow uploads to Google Play or App Store Connect.

## Release preparation

Update the production versions with one command:

```sh
yarn release:prod:prep 2.0.24 318
```

Review the generated diff, commit it, and create the suggested `wcash-<version>-<build>` tag. Run the unsigned candidate workflow on that tag. The candidate workflow does not accept beta tags because its metadata, Android flavor, and iOS scheme are production-channel inputs. Promote a source revision only after Regtest and public Testnet lifecycle tests pass.

## Local checks

From the repository root:

```sh
node scripts/ci/verify-wcash-dependencies.mjs
node scripts/ci/verify-workflow-actions.mjs
node scripts/assert-upstream-ui-parity.mjs
yarn test --runInBand
yarn typecheck
yarn lint:check
yarn prettier:check
```

From `rust` with Rust 1.91.0:

```sh
cargo test --locked -p wcash-mobile-adapter -p wcash-mobile-ffi
cargo clippy --locked -p wcash-mobile-adapter -p wcash-mobile-ffi --all-targets -- -D warnings
```

The Android Local Regtest command builds the same ARM64 prodDebug package used by CI:

```sh
node rust/android/build_android_local.mjs arm64 --regtest-qa-apk
```

The iOS app requires full Xcode. CI provides the controlled Xcode 26.6 environment and the simulator artifact.
