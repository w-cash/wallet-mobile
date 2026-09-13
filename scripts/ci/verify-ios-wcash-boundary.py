#!/usr/bin/env python3

import plistlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def require(text: str, token: str, source: str) -> None:
    if token not in text:
        raise SystemExit(f"{source} is missing {token!r}")


def reject(text: str, token: str, source: str) -> None:
    if token in text:
        raise SystemExit(f"{source} contains legacy fixture {token!r}")


swift_path = ROOT / "ios/ZingoTests/ZingoTest.swift"
swift = swift_path.read_text()
for token in (
    'static let endpoint = "http://127.0.0.1:48234"',
    'static let privateAddressPrefix = "wuregtest1"',
    'static let transparentAddressPrefix = "WR"',
    'static let consensusBranchID = "c3a6678a"',
    'static let ticker = "TWC"',
    "throw XCTSkip",
    "try confirm()",
    "let value: Int64",
    "try getValueTransfers()",
    "try initFromBytes(",
):
    require(swift, token, str(swift_path))

for token in (
    "http://10.0.2.2:20000",
    "http://127.0.0.1:20000",
    "uviewregtest1",
    'address: "uregtest1',
    "texregtest1",
    "t1dUDJ62",
    "u1gsqvq",
):
    reject(swift, token, str(swift_path))
reject(swift, "SIMCTL_CHILD_", str(swift_path))

server_path = ROOT / "app/uris/serverUris.ts"
require(server_path.read_text(), "http://127.0.0.1:48234", str(server_path))

adapter_path = ROOT / "rust/wcash-mobile-adapter/src/lib.rs"
adapter = adapter_path.read_text()
require(adapter, '"consensus_branch_id": wallet_network.branch_id_hex()', str(adapter_path))
require(adapter, '"TWC"', str(adapter_path))

plist_path = ROOT / "ios/Zingo/Info.plist"
with plist_path.open("rb") as plist_file:
    plist = plistlib.load(plist_file)
if plist.get("UILaunchStoryboardName") != "LaunchScreen":
    raise SystemExit("ios/Zingo/Info.plist must select LaunchScreen")
if not (ROOT / "ios/Zingo/LaunchScreen.storyboard").is_file():
    raise SystemExit("ios/Zingo/LaunchScreen.storyboard is missing")

podfile_path = ROOT / "ios/Podfile"
podfile = podfile_path.read_text()
for token in (
    "d1777a66de37245b31949b0ed93d8d2e292bfe41f9f671c75b349c53da152880",
    "Digest::SHA256.file(archive.path).hexdigest",
    "raise Pod::Informative",
):
    require(podfile, token, str(podfile_path))
reject(podfile, "curl -L '#{url}' | tar", str(podfile_path))

rpc_path = ROOT / "ios/RPCModule.swift"
rpc = rpc_path.read_text()
require(rpc, ".posixPermissions: 0o700", str(rpc_path))
require(
    rpc,
    "FileProtectionType.completeUntilFirstUserAuthentication",
    str(rpc_path),
)

runner_path = ROOT / "scripts/ci/ios_integration_tests_ci.sh"
runner = runner_path.read_text()
for token in (
    "prepare-ios-xctestrun.py",
    "verify-ios-xcresult.py",
    "test-without-building",
    "testCreateRestoreSyncReceiveAndHistory",
):
    require(runner, token, str(runner_path))
reject(runner, "SIMCTL_CHILD_", str(runner_path))

builder_path = ROOT / "scripts/ios_integration_tests.sh"
builder = builder_path.read_text()
for token in (
    "build-for-testing",
    "ios_integration_tests_ci.sh",
    "testCreateRestoreSyncReceiveAndHistory",
):
    require(builder, token, str(builder_path))
reject(builder, "SIMCTL_CHILD_", str(builder_path))

xctestrun_helper_path = ROOT / "scripts/ci/prepare-ios-xctestrun.py"
xctestrun_helper = xctestrun_helper_path.read_text()
for token in (
    '"EnvironmentVariables"',
    '"TestingEnvironmentVariables"',
    '"WCASH_IOS_LIVE_REGTEST"',
    '"WCASH_IOS_FUNDED_TEST_SEED"',
    "source.parent != output.parent",
):
    require(xctestrun_helper, token, str(xctestrun_helper_path))

xcresult_helper_path = ROOT / "scripts/ci/verify-ios-xcresult.py"
xcresult_helper = xcresult_helper_path.read_text()
for token in (
    'summary.get("totalTestCount")',
    'summary.get("passedTests")',
    'summary.get("skippedTests")',
    "total < 1 or passed < 1",
    "args.live and skipped != 0",
):
    require(xcresult_helper, token, str(xcresult_helper_path))

print("iOS Wcash boundary verification passed")
