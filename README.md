# Wcash Wallet Mobile

This repository contains the Android and iOS Wcash Wallet applications. The
apps use React Native with a Rust wallet backend derived from Zingo Mobile and
Zingolib.

> **Developer preview:** Wcash Wallet Mobile has no supported consumer release.
> The published Mainnet packages are engineering candidates. They are unsafe
> for material funds.

## Current package status

The latest published candidate is
[`wcash-2.0.23-317`](https://github.com/w-cash/wallet-mobile/releases/tag/wcash-2.0.23-317),
built from
[`f136a09d7b4959ee800dcef6d4cb9a0b4b39b1da`](https://github.com/w-cash/wallet-mobile/commit/f136a09d7b4959ee800dcef6d4cb9a0b4b39b1da)
for Wcash Mainnet.

| Package | Status | Installation |
| --- | --- | --- |
| Android ARM64 `prodDebug` APK | Developer candidate signed with the public Android debug certificate | Manual installation for development and review |
| iOS ARM64 simulator ZIP | Unsigned compile and test artifact | Xcode simulator use by developers |
| iOS ARM64 device ZIP | Unsigned compile and test artifact | Requires Apple signing and provisioning before device installation |

The Android package uses the production application ID, but its `prodDebug`
build type and public debug certificate provide no production publisher
identity. The iOS archives are not ordinary installable consumer downloads.
Wcash has no Google Play, TestFlight, or Apple App Store release.

The candidate connects to the hard-coded Mainnet wallet service at
`http://mainnet.zecwec.com:48234`. This plaintext endpoint provides no TLS
server authentication or transport encryption. Authenticated Mainnet
transport remains a release blocker.

## Release records and verification

The candidate release contains these records:

- [`MANIFEST.json`](https://github.com/w-cash/wallet-mobile/releases/download/wcash-2.0.23-317/MANIFEST.json) records the network, version, build, full source revision, toolchains, artifact sizes, signing state, and SHA-256 values.
- [`SHA256SUMS`](https://github.com/w-cash/wallet-mobile/releases/download/wcash-2.0.23-317/SHA256SUMS) contains the SHA-256 value for each package.
- [`SOURCE.txt`](https://github.com/w-cash/wallet-mobile/releases/download/wcash-2.0.23-317/SOURCE.txt) identifies the release tag, source revision, and package status.

The published package digests are:

| Package | SHA-256 |
| --- | --- |
| Android ARM64 `prodDebug` APK | `ad41c67ed080d9a394bb80dacd653c195678144db261732fa967ab93d1b2fcc4` |
| iOS ARM64 device compile ZIP | `3a8987229ba2e7f2c64a99f18fc833c0e953368078b44ff61502d1ebccf81fa9` |
| iOS ARM64 simulator ZIP | `0debbc9dee7fb708e2095f153085bdf0014dde67a55e5926b8b904dc35f9f8d4` |

After downloading a package and `SHA256SUMS`, calculate its digest and compare
the complete hexadecimal value with the matching line:

```sh
shasum -a 256 <downloaded-package>
```

The checksum detects corruption or modification relative to the published
checksum file. It does not authenticate the publisher without a trusted
signature or another trusted root. The release tag has no cryptographic
signature. This release has no signed provenance or SBOM. The Android debug
signature and the unsigned iOS archives provide no production publisher
authentication.

## Build and test

Install Node.js 22, Yarn 1.22.22, and Rust 1.91.0. Platform builds also need
the toolchains listed in
[`docs/release_quickstart.md`](./docs/release_quickstart.md).

Run the source checks from the repository root:

```sh
corepack enable
yarn install --frozen-lockfile
yarn docs:check
yarn test --runInBand
yarn typecheck
yarn lint:check
yarn prettier:check
```

Run the Wcash adapter and native interface tests from `rust/`:

```sh
cargo test --locked -p wcash-mobile-adapter -p wcash-mobile-ffi -p rustios
cargo clippy --locked -p wcash-mobile-adapter -p wcash-mobile-ffi --all-targets -- -D warnings
```

Use the
[`Android developer guide`](./docs/android_developer_quickstart.md) and
[`iOS developer guide`](./docs/ios_developer_quickstart.md) for local builds.
The local live suites require a Wcash Regtest service.

## Source relationships

The Wcash mobile boundary depends on immutable revisions of
[`w-cash/wallet-core`](https://github.com/w-cash/wallet-core) and
[`w-cash/wolf`](https://github.com/w-cash/wolf). The exact revisions are in
`rust/Cargo.toml` and `rust/Cargo.lock`.

This project retains work from
[`zingolabs/zingo-mobile`](https://github.com/zingolabs/zingo-mobile),
[`zingolabs/zingolib`](https://github.com/zingolabs/zingolib), and other Zcash
projects. Their names remain in internal paths and dependencies where the fork
preserves compatibility. Upstream product pages, stores, and support routes do
not serve Wcash Wallet users.

Read [`SECURITY.md`](./SECURITY.md) for private vulnerability reporting and
[`SUPPORT.md`](./SUPPORT.md) for public support and diagnostic redaction.

## License

The repository uses the MIT License. See [`LICENSE`](./LICENSE) for the
preserved copyright and terms.
