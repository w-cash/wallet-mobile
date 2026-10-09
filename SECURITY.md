# Wcash Wallet Mobile security policy

## Supported versions

Wcash Wallet Mobile is a developer preview. The project has no supported
consumer version on Google Play, TestFlight, or the Apple App Store. Current
GitHub prerelease packages receive no consumer support or security SLA.

The latest published package is the unsupported Mainnet candidate
[`wcash-2.0.23-317`](https://github.com/w-cash/wallet-mobile/releases/tag/wcash-2.0.23-317).
Its Android APK is debug-signed and its iOS archives are unsigned. The
candidate also uses a plaintext Mainnet wallet service.

## Private vulnerability reports

Use
[GitHub Private Vulnerability Reporting](https://github.com/w-cash/wallet-mobile/security/advisories/new)
for vulnerabilities in this repository. Include the affected source revision,
platform, impact, reproduction steps, and a proof of concept when available.

The repository owner must enable and verify Private Vulnerability Reporting in
the repository security settings. The owner must test that the link presents a
private report form before announcing this route as operational. If the form
is unavailable, do not put vulnerability details in a public issue. Wcash has
no published private security mailbox.

Do not include a recovery phrase, spending key, full viewing key, wallet file,
real address, transaction material, or other wallet secret in a report. Use a
fresh Regtest wallet and sanitized logs for reproduction.

The project has not published guaranteed acknowledgement or remediation times.
The maintainers should agree on a coordinated disclosure date with the
reporter after they receive and assess a private report.

## Scope

Security reports can cover:

- recovery phrase, key, wallet file, or sensitive metadata exposure
- unauthorized signing, transaction mutation, or fund movement
- network identity, endpoint authentication, or transport failures
- cross-network address or transaction acceptance
- dependency, native interface, application link, or platform storage flaws
- release artifact, signing, update, or build provenance failures

Report an upstream flaw to this repository when it affects the Wcash build.
The Wcash maintainers can coordinate a fix with the upstream project. They
must obtain the reporter's permission before sharing private report details or
identity outside the Wcash security team.

Use [public GitHub issues](https://github.com/w-cash/wallet-mobile/issues) for
non-sensitive bugs and build problems. Follow [`SUPPORT.md`](./SUPPORT.md)
before attaching diagnostics.
