# Wcash Wallet for iOS

The iOS build requires macOS with full Xcode. The controlled CI build uses the GitHub `macos-26` ARM64 image and Xcode 26.6.

## Toolchain

Install these versions:

- Xcode 26.6 with an iOS Simulator runtime
- Node.js 22.18.0 and Yarn 1.22.22
- Rust 1.91.0 from `rust-toolchain.toml`
- Ruby 3.3.12 and Bundler 2.3.18 from `Gemfile.lock`
- CocoaPods from the Ruby bundle
- Protobuf

Select Xcode and install the Rust targets:

```sh
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

Install the locked project dependencies from the repository root:

```sh
corepack enable
yarn install --frozen-lockfile
bundle install
```

The install applies `patches/react-native-keychain+10.0.0.patch`, which carries upstream commit `af7612e251d744891d22a7ef935d95052f1328c8` onto the exact npm package for React Native's New Architecture.

## Build

Build the Wcash FFI, the Apple XCFramework, the Swift bindings, and the CocoaPods workspace:

```sh
yarn rust:ios
bundle exec pod install --project-directory=ios --deployment
```

`yarn rust:ios` creates `ios/Zingolib.xcframework` with an ARM64 device slice and ARM64/x86_64 simulator slices. It also creates `ios/zingo.swift` from the Wcash FFI contract.

Open `ios/Zingo.xcworkspace` in Xcode to run the app. The unsigned CI-equivalent simulator build is:

```sh
xcodebuild \
  -workspace ios/Zingo.xcworkspace \
  -scheme Zingo \
  -configuration Release \
  -sdk iphonesimulator \
  -destination 'generic/platform=iOS Simulator' \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  build
```

That unsigned package is a compile artifact. Build a runnable simulator app with Xcode's local ad-hoc signature:

```sh
xcodebuild \
  -workspace ios/Zingo.xcworkspace \
  -scheme Zingo \
  -configuration Release \
  -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.5' \
  -derivedDataPath ios/build/ReleaseSimulatorDerived \
  build

xcrun simctl install booted \
  ios/build/ReleaseSimulatorDerived/Build/Products/Release-iphonesimulator/Zingo.app
```

The ad-hoc signature gives the simulator Keychain an application identity. A signing-disabled package returns Keychain OSStatus `-34018` when installed.

The CI also links the complete app against the `iphoneos` device slice:

```sh
RCT_NO_LAUNCH_PACKAGER=1 xcodebuild \
  -workspace ios/Zingo.xcworkspace \
  -scheme Zingo \
  -configuration Release \
  -sdk iphoneos \
  -destination 'generic/platform=iOS' \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=NO \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  build
```

This proves the app and Wcash XCFramework link for an ARM64 iPhone target. Installing it on a phone still requires an Apple development or distribution signing identity and provisioning profile.

## XCTest paths

The default command builds and runs the full `ZingoTests` target:

```sh
./scripts/ios_integration_tests.sh
```

The default suite runs the static Wcash identity, bridge, and wallet-file tests. The live Regtest class calls `XCTSkip` unless the command enables it. Select a class or method with `-e`:

```sh
./scripts/ios_integration_tests.sh -e ZingoTests/WcashIOSIdentityTests
```

CI builds the test bundle once:

```sh
xcodebuild build-for-testing \
  -workspace ios/Zingo.xcworkspace \
  -scheme Zingo \
  -configuration Debug \
  -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.5' \
  -derivedDataPath ios/build/DerivedData \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES
```

The static runner then executes the generated test bundle without rebuilding it:

```sh
./scripts/ci/ios_integration_tests_ci.sh
```

This script uses the `.xctestrun` file under `ios/build/DerivedData/Build/Products` and calls `xcodebuild test-without-building`.

## Local Regtest

Start Wcash CompactTxStreamer on the Mac at `127.0.0.1:48234`, then run:

```sh
./scripts/ios_integration_tests.sh -l
```

An iOS Simulator shares the Mac network stack. Its `127.0.0.1:48234` connection reaches the Wcash service on the Mac. The test script checks that port before it starts XCTest and forwards `WCASH_IOS_LIVE_REGTEST=1` into the simulator process.

The live class creates a wallet, restores it, syncs to the current tip, verifies Wcash receiver prefixes, reads the balance, and checks history. Supply a disposable funded Regtest phrase to add the broadcast and pending-history test:

```sh
WCASH_IOS_FUNDED_TEST_SEED='24 disposable words' \
  ./scripts/ios_integration_tests.sh -l
```

The funded phrase must contain at least `0.002 TWC` in confirmed Ironwood funds. The suite sends `0.001 TWC` to its own receiver and records the transaction in pending history. Use a test-only phrase.

Override the simulator when needed:

```sh
WCASH_IOS_DESTINATION='platform=iOS Simulator,name=iPhone 17,OS=latest' \
  ./scripts/ios_integration_tests.sh
```

The live test endpoint is simulator-specific. A physical iPhone resolves `127.0.0.1` to the phone and needs an approved reachable Wcash endpoint.

## Candidate and production gates

The pull request workflow builds compile-only unsigned ARM64 Release apps for the simulator and generic iPhone target and retains them for three days. The unsigned candidate workflow packages both iOS compile artifacts with the Android counterpart, manifest, and SHA-256 checksums for 14 days.

Production signing uses the `wcash-mobile-production` GitHub environment. Configure required reviewers before adding the Apple distribution certificate, provisioning profile, App Store Connect credential, registered bundle ID, privacy record, and support metadata. `.github/workflows/mobile-production-signing.yaml` currently stops before signing or store delivery.
