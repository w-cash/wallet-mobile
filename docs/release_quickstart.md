# Wcash Wallet mobile build and release gate

This repository retains parts of the upstream Zingo Mobile interface. Wcash
builds use the product name `Wcash Wallet`.

| Channel    | iOS bundle ID                 | Android application ID        | Display name   |
| ---------- | ----------------------------- | ----------------------------- | -------------- |
| Production | `com.wcashwallet.wallet`      | `com.wcashwallet.wallet`      | `Wcash Wallet` |
| Beta       | `com.wcashwallet.wallet.beta` | `com.wcashwallet.wallet.beta` | `Wcash Wallet` |

The repository can package Mainnet developer candidates. A supported consumer
release remains blocked on authenticated transport, qualification, production
signing, publisher authentication, and store review.

## Pull request checks

`.github/workflows/wcash-pr-qa.yaml` runs these independent jobs:

1. The source job checks the upstream UI parity, JavaScript tests, TypeScript, ESLint, Prettier, Rustfmt, Clippy, and the Wcash adapter and FFI tests.
2. The Android job builds the ARM64 Wcash FFI, runs the prodDebug JVM tests and lint, and assembles a complete prodDebug APK with its JavaScript bundle. It checks the package ID, display name, ABI, native library count, signing envelope, and absence of the legacy Nym native library.
3. The iOS job uses the GitHub `macos-26` ARM64 image, Xcode 26.6, Ruby 3.3.12, Bundler 2.3.18, and Rust 1.91.0. It builds the Wcash XCFramework, installs the locked CocoaPods graph, runs the static XCTest suite on the iPhone 17/iOS 26.5 simulator, and links complete unsigned ARM64 Release apps for the simulator and a physical-device target. It checks the bundle ID, display name, JavaScript bundle, architecture, signing state, and absence of the legacy Nym native library.

The pull request APK and both iOS ZIPs remain available for three days. The device ZIP proves the app links for `iphoneos`; installation on a phone still requires Apple signing and provisioning.

## Developer candidates

Run **Wcash mobile unsigned candidate** from GitHub Actions on a reviewed
commit or a `wcash-<version>-<build>` release tag. The workflow records
`mainnet` in the filename and manifest. It builds these files:

The workflow produces one aggregate artifact for 14 days:

```text
Wcash-Wallet-mainnet-<version>-<build>-<sha>-android-arm64-prodDebug.apk
Wcash-Wallet-mainnet-<version>-<build>-<sha>-ios-arm64-simulator-unsigned.zip
Wcash-Wallet-mainnet-<version>-<build>-<sha>-ios-arm64-device-compile-unsigned.zip
MANIFEST.json
SHA256SUMS
```

The manifest records the full source revision, network, version, build number,
compiler versions, platform, signing state, file size, and SHA-256 digest. The
workflow checks every digest before upload. A tag run also publishes
`SOURCE.txt` and creates a GitHub prerelease.

The Android artifact uses the `prodDebug` build type and the public Android
debug certificate. The iOS archives have signing disabled. The device archive
proves that the source links for `iphoneos`; it requires Apple signing and
provisioning before installation on an iPhone. These packages are engineering
artifacts rather than a supported consumer release.

The latest public candidate is
[`wcash-2.0.23-317`](https://github.com/w-cash/wallet-mobile/releases/tag/wcash-2.0.23-317).
It was built from
[`f136a09d7b4959ee800dcef6d4cb9a0b4b39b1da`](https://github.com/w-cash/wallet-mobile/commit/f136a09d7b4959ee800dcef6d4cb9a0b4b39b1da).
Its
[`MANIFEST.json`](https://github.com/w-cash/wallet-mobile/releases/download/wcash-2.0.23-317/MANIFEST.json),
[`SHA256SUMS`](https://github.com/w-cash/wallet-mobile/releases/download/wcash-2.0.23-317/SHA256SUMS),
and
[`SOURCE.txt`](https://github.com/w-cash/wallet-mobile/releases/download/wcash-2.0.23-317/SOURCE.txt)
are release assets. Its manifest records Rust 1.91.0 and Xcode 27.0. The
current candidate workflow pins Xcode 26.6, so a new run will record a
different Apple toolchain from this published candidate.

The SHA-256 values detect corruption or modification relative to the checksum
file. They do not authenticate the publisher without a trusted signature or
another trusted root. The release tag has no cryptographic signature. The
current release includes no signed provenance or SBOM.

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

`.github/workflows/mobile-production-signing.yaml` uses the protected
`wcash-mobile-production` GitHub environment. Configure required reviewers on
that environment before adding credentials. The current gate validates an
exact Wcash release tag and an explicit confirmation phrase, then exits before
signing.

Keep Android prodRelease and AAB work in this protected workflow. Keep Apple
distribution signing and App Store packaging in this protected workflow.
Candidate and pull request jobs use prodDebug Android signing and
signing-disabled iOS builds. The repository has no Google Play, TestFlight, or
Apple App Store release. No workflow uploads to Google Play or App Store
Connect.

## Release preparation

Update the production versions with one command:

```sh
yarn release:prod:prep 2.0.24 318
```

Review the generated diff, commit it, and create the suggested
`wcash-<version>-<build>` tag. Run the unsigned candidate workflow on that tag.
The candidate workflow accepts the production application identity and creates
a developer prerelease. Promotion requires the documented Regtest and public
Testnet lifecycle tests, authenticated Mainnet transport, production signing,
and release review.

## Local checks

From the repository root:

```sh
node scripts/ci/verify-wcash-dependencies.mjs
node scripts/ci/verify-workflow-actions.mjs
node scripts/ci/verify-public-docs.mjs
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
