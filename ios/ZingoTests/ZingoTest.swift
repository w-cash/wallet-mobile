//
//  ZingoTest.swift
//  ZingoTests
//
//  Created by Juan Carlos Carmona Calvo on 5/2/24.
//

import Foundation
import UIKit

import React
import XCTest

enum WcashRegtest {
    static let endpoint = "http://127.0.0.1:48234"
    static let chainHint = "regtest"
    static let privateAddressPrefix = "wuregtest1"
    static let transparentAddressPrefix = "WR"
    static let consensusBranchID = "c3a6678a"
    static let ticker = "TWC"
    static let liveTestEnvironment = "WCASH_IOS_LIVE_REGTEST"
    static let fundedSeedEnvironment = "WCASH_IOS_FUNDED_TEST_SEED"
}

enum Seeds {
    static let HOSPITAL = "hospital museum valve antique skate museum unfold vocal weird milk scale social vessel identify crowd hospital control album rib bulb path oven civil tank"
}

struct RecoveryInfo: Codable {
    let seed_phrase: String
    let birthday: UInt64
    let chain_name: String
}

struct UnifiedAddress: Codable, Equatable {
    let account: UInt64
    let address_index: UInt64
    let has_orchard: Bool
    let has_sapling: Bool
    let has_transparent: Bool
    let encoded_address: String
}

struct TransparentAddress: Codable, Equatable {
    let account: UInt64
    let address_index: UInt64
    let scope: String
    let encoded_address: String
}

struct Info: Codable {
    let version: String
    let git_commit: String
    let server_uri: String
    let vendor: String
    let taddr_support: Bool
    let chain_name: String
    let sapling_activation_height: UInt64
    let consensus_branch_id: String
    let latest_block_height: UInt64
    let ironwood_activation_height: UInt64
}

struct Height: Codable {
    let height: UInt64
}

struct Balance: Codable {
    let total_orchard_balance: UInt64
    let confirmed_orchard_balance: UInt64
    let unconfirmed_orchard_balance: UInt64
    let total_sapling_balance: UInt64
    let confirmed_sapling_balance: UInt64
    let unconfirmed_sapling_balance: UInt64
    let total_transparent_balance: UInt64
    let confirmed_transparent_balance: UInt64
    let unconfirmed_transparent_balance: UInt64
    let total_ironwood_balance: UInt64
    let confirmed_ironwood_balance: UInt64
    let unconfirmed_ironwood_balance: UInt64
}

struct SendRequest: Codable {
    let address: String
    let amount: UInt64
    let memo: String?
}

struct ValueTransfer: Codable {
    let txid: String
    let status: String
    let kind: String
    let value: Int64
    let transaction_fee: UInt64?
    let recipient_address: String?
}

struct ValueTransfers: Codable {
    let value_transfers: [ValueTransfer]
    let total: UInt64
}

struct ParseResult: Codable, Equatable {
    let status: String
    let chain_name: String
    let address_kind: String
    let receivers_available: [String]
    let shielded_only_ua: String
}

struct SendOutcome: Codable {
    let txids: [String]
}

private enum WcashTestError: Error {
    case syncTimeout
}

private func decodeJSON<T: Decodable>(_ json: String) throws -> T {
    try JSONDecoder().decode(T.self, from: Data(json.utf8))
}

private func setCryptoProvider() throws {
    _ = try setCryptoDefaultProviderToRing()
}

private func testEnvironment(_ name: String) -> String? {
    ProcessInfo.processInfo.environment[name]
}

private func requireLiveRegtest() throws {
    guard testEnvironment(WcashRegtest.liveTestEnvironment) == "1" else {
        throw XCTSkip("Set WCASH_IOS_LIVE_REGTEST=1 and run the local Wcash indexer on 127.0.0.1:48234")
    }
}

private func selectWalletDirectory() throws -> URL {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("wcash-ios-tests-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(
        at: directory,
        withIntermediateDirectories: true,
        attributes: [.posixPermissions: 0o700]
    )
    try FileManager.default.setAttributes(
        [.posixPermissions: 0o700],
        ofItemAtPath: directory.path
    )
    _ = try setWalletDirectory(directory: directory.path)
    return directory
}

private func serverInfo() throws -> Info {
    let info: Info = try decodeJSON(try infoServer())
    XCTAssertEqual(info.server_uri, WcashRegtest.endpoint)
    XCTAssertEqual(info.vendor, "Wcash Wallet")
    XCTAssertEqual(info.chain_name, WcashRegtest.chainHint)
    XCTAssertEqual(info.consensus_branch_id, WcashRegtest.consensusBranchID)
    XCTAssertEqual(info.ironwood_activation_height, 1)
    XCTAssertGreaterThan(info.latest_block_height, 0)
    return info
}

private func receiveAddresses() throws -> (unified: UnifiedAddress, transparent: TransparentAddress) {
    let unified: [UnifiedAddress] = try decodeJSON(try getUnifiedAddresses())
    let transparent: [TransparentAddress] = try decodeJSON(try getTransparentAddresses())
    let privateAddress = try XCTUnwrap(unified.first)
    let miningAddress = try XCTUnwrap(transparent.first)
    XCTAssertTrue(privateAddress.encoded_address.hasPrefix(WcashRegtest.privateAddressPrefix))
    XCTAssertTrue(miningAddress.encoded_address.hasPrefix(WcashRegtest.transparentAddressPrefix))
    XCTAssertTrue(privateAddress.has_orchard)
    XCTAssertFalse(privateAddress.has_sapling)
    XCTAssertFalse(privateAddress.has_transparent)
    XCTAssertEqual(miningAddress.scope, "external")
    return (privateAddress, miningAddress)
}

private func syncToTip(_ tip: UInt64, timeoutSeconds: TimeInterval = 120) throws {
    XCTAssertEqual(try runSync(), "Launching sync task...")
    let deadline = Date().addingTimeInterval(timeoutSeconds)
    while Date() < deadline {
        let poll = try pollSync()
        if poll == "Sync task is not complete" {
            Thread.sleep(forTimeInterval: 0.25)
            continue
        }
        let completion = try JSONSerialization.jsonObject(with: Data(poll.utf8)) as? [String: Any]
        XCTAssertNotNil(completion?["sync_complete"])
        let height: Height = try decodeJSON(try getLatestBlockWallet())
        XCTAssertEqual(height.height, tip)
        return
    }
    throw WcashTestError.syncTimeout
}

final class WcashIOSIdentityTests: XCTestCase {
    func testLocalRegtestProfile() {
        XCTAssertEqual(WcashRegtest.endpoint, "http://127.0.0.1:48234")
        XCTAssertEqual(WcashRegtest.privateAddressPrefix, "wuregtest1")
        XCTAssertEqual(WcashRegtest.transparentAddressPrefix, "WR")
        XCTAssertEqual(WcashRegtest.consensusBranchID, "c3a6678a")
        XCTAssertEqual(WcashRegtest.ticker, "TWC")
    }

    func testNativeVersionUsesTheWcashRuntime() throws {
        let version = try getVersion()
        XCTAssertTrue(version.hasPrefix("wcash-wallet-core-"))
        XCTAssertTrue(version.hasSuffix("-wcash-mobile-adapter"))
    }

    func testMainnetFailsBeforeWalletCreation() {
        XCTAssertThrowsError(
            try initFromSeed(
                seed: Seeds.HOSPITAL,
                birthday: 1,
                serveruri: "https://example.invalid",
                chainhint: "main",
                performancelevel: "Medium",
                minconfirmations: 1
            )
        ) { error in
            guard case ZingolibError.InvalidInput(let message) = error else {
                return XCTFail("Expected InvalidInput, got \(error)")
            }
            XCTAssertTrue(message.contains("Mainnet"))
        }
    }

    func testWatchOnlyRestoreReportsItsBoundary() {
        XCTAssertThrowsError(
            try initFromUfvk(
                ufvk: "unsupported",
                birthday: 1,
                serveruri: WcashRegtest.endpoint,
                chainhint: WcashRegtest.chainHint,
                performancelevel: "Medium",
                minconfirmations: 1
            )
        ) { error in
            guard case ZingolibError.InvalidInput(let message) = error else {
                return XCTFail("Expected InvalidInput, got \(error)")
            }
            XCTAssertTrue(message.contains("UFVK/watch-only restore"))
        }
    }
}

final class WcashLocalRegtestTests: XCTestCase {
    func testCreateRestoreSyncReceiveAndHistory() throws {
        try requireLiveRegtest()
        try setCryptoProvider()

        let createdDirectory = try selectWalletDirectory()
        defer { try? FileManager.default.removeItem(at: createdDirectory) }
        let created: RecoveryInfo = try decodeJSON(
            try initNew(
                serveruri: WcashRegtest.endpoint,
                birthday: 0,
                chainhint: WcashRegtest.chainHint,
                performancelevel: "Medium",
                minconfirmations: 1
            )
        )
        XCTAssertEqual(created.chain_name, WcashRegtest.chainHint)
        XCTAssertEqual(created.seed_phrase.split(separator: " ").count, 24)
        let createdInfo = try serverInfo()
        let createdAddresses = try receiveAddresses()
        try syncToTip(createdInfo.latest_block_height)

        let restoredDirectory = try selectWalletDirectory()
        defer { try? FileManager.default.removeItem(at: restoredDirectory) }
        let restored: RecoveryInfo = try decodeJSON(
            try initFromSeed(
                seed: created.seed_phrase,
                birthday: UInt32(created.birthday),
                serveruri: WcashRegtest.endpoint,
                chainhint: WcashRegtest.chainHint,
                performancelevel: "Medium",
                minconfirmations: 1
            )
        )
        XCTAssertEqual(restored.seed_phrase, created.seed_phrase)
        XCTAssertEqual(restored.chain_name, WcashRegtest.chainHint)
        let restoredInfo = try serverInfo()
        let restoredAddresses = try receiveAddresses()
        XCTAssertEqual(restoredAddresses.unified, createdAddresses.unified)
        XCTAssertEqual(restoredAddresses.transparent, createdAddresses.transparent)
        try syncToTip(restoredInfo.latest_block_height)

        let parsed: ParseResult = try decodeJSON(
            try parseAddress(address: restoredAddresses.unified.encoded_address)
        )
        XCTAssertEqual(parsed.status, "success")
        XCTAssertEqual(parsed.chain_name, WcashRegtest.chainHint)
        XCTAssertEqual(parsed.address_kind, "unified")
        XCTAssertEqual(parsed.shielded_only_ua, restoredAddresses.unified.encoded_address)

        let history: ValueTransfers = try decodeJSON(try getValueTransfers())
        XCTAssertEqual(history.total, UInt64(history.value_transfers.count))
        let balance: Balance = try decodeJSON(try getBalance())
        XCTAssertEqual(balance.confirmed_ironwood_balance, 0)
        XCTAssertEqual(balance.confirmed_transparent_balance, 0)
    }

    func testFundedWalletBroadcastPersistsInPendingHistory() throws {
        try requireLiveRegtest()
        guard let seed = testEnvironment(WcashRegtest.fundedSeedEnvironment),
              seed.split(separator: " ").count == 24 else {
            throw XCTSkip("Set WCASH_IOS_FUNDED_TEST_SEED to a disposable funded Regtest phrase")
        }
        try setCryptoProvider()
        let directory = try selectWalletDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        _ = try initFromSeed(
            seed: seed,
            birthday: 1,
            serveruri: WcashRegtest.endpoint,
            chainhint: WcashRegtest.chainHint,
            performancelevel: "Medium",
            minconfirmations: 1
        )
        let info = try serverInfo()
        try syncToTip(info.latest_block_height)
        let balance: Balance = try decodeJSON(try getBalance())
        guard balance.confirmed_ironwood_balance >= 200_000 else {
            throw XCTSkip("The disposable fixture needs at least 0.002 TWC in confirmed Ironwood funds")
        }
        let recipient = try receiveAddresses().unified.encoded_address
        let amount: UInt64 = 100_000
        let request = SendRequest(address: recipient, amount: amount, memo: nil)
        let requestJSON = String(decoding: try JSONEncoder().encode([request]), as: UTF8.self)
        let proposal = try JSONSerialization.jsonObject(
            with: Data(try send(sendJson: requestJSON).utf8)
        ) as? [String: Any]
        let fee = proposal?["fee"] as? NSNumber
        XCTAssertGreaterThan(fee?.uint64Value ?? 0, 0)

        let outcome: SendOutcome = try decodeJSON(try confirm())
        let txid = try XCTUnwrap(outcome.txids.first)
        XCTAssertEqual(txid.count, 64)
        let pending: ValueTransfers = try decodeJSON(try getValueTransfers())
        let sent = try XCTUnwrap(pending.value_transfers.first { $0.txid == txid })
        XCTAssertEqual(sent.kind, "sent")
        XCTAssertEqual(abs(sent.value), Int64(amount))
        XCTAssertEqual(sent.recipient_address, recipient)
        XCTAssertTrue(["transmitted", "calculated"].contains(sent.status))

        let wallet = try XCTUnwrap(try saveWalletBytes())
        let reopenedDirectory = try selectWalletDirectory()
        defer { try? FileManager.default.removeItem(at: reopenedDirectory) }
        let reopened: RecoveryInfo = try decodeJSON(
            try initFromBytes(
                walletBytes: wallet,
                serveruri: WcashRegtest.endpoint,
                chainhint: WcashRegtest.chainHint,
                performancelevel: "Medium",
                minconfirmations: 1
            )
        )
        XCTAssertEqual(reopened.chain_name, WcashRegtest.chainHint)
        let reopenedHistory: ValueTransfers = try decodeJSON(try getValueTransfers())
        let restoredSent = try XCTUnwrap(
            reopenedHistory.value_transfers.first { $0.txid == txid }
        )
        XCTAssertEqual(restoredSent.kind, "sent")
        XCTAssertEqual(restoredSent.status, sent.status)
        XCTAssertEqual(abs(restoredSent.value), Int64(amount))
    }
}

/// The bridge-outcome contract for every migrated FFI (zingo-mobile#1151):
/// whether a call succeeded is knowable from the channel of its result —
/// resolved versus rejected — never from its content, and a rejection's
/// code is exactly the thrown ZingolibError variant's name, the stable
/// code shared by every bridge. One case per contract variant. These are
/// the Swift twins of the Rust init_error_channel_tests, the Kotlin
/// FfiOutcomeTest, and the TypeScript ffiOutcome tests.
class FfiOutcomeTests: XCTestCase {
    // Every contract variant, paired with its stable rejection code.
    private let contractVariants: [(error: ZingolibError, code: String)] = [
        (ZingolibError.LightclientNotInitialized(message: "boom"), "LightclientNotInitialized"),
        (ZingolibError.LightclientLockPoisoned(message: "boom"), "LightclientLockPoisoned"),
        (ZingolibError.Panic(message: "boom"), "Panic"),
        (ZingolibError.Save(message: "boom"), "Save"),
        (ZingolibError.Init(message: "boom"), "Init"),
        (ZingolibError.Sync(message: "boom"), "Sync"),
        (ZingolibError.Rescan(message: "boom"), "Rescan"),
        (ZingolibError.Read(message: "boom"), "Read"),
        (ZingolibError.Send(message: "boom"), "Send"),
        (ZingolibError.Shield(message: "boom"), "Shield"),
        (ZingolibError.InvalidInput(message: "boom"), "InvalidInput"),
        (ZingolibError.Wallet(message: "boom"), "Wallet"),
        (ZingolibError.Indexer(message: "boom"), "Indexer"),
        (ZingolibError.Offline(message: "boom"), "Offline"),
        (ZingolibError.SideChannelPoisoned(message: "boom"), "SideChannelPoisoned"),
        (ZingolibError.MigrationNotInProgress(message: "boom"), "MigrationNotInProgress"),
        (ZingolibError.MigrationAlreadyInProgress(message: "boom"), "MigrationAlreadyInProgress"),
        (ZingolibError.MigrationConsentStale(message: "boom"), "MigrationConsentStale"),
        (ZingolibError.MigrationCadenceFixed(message: "boom"), "MigrationCadenceFixed"),
        (ZingolibError.MigrationSplit(message: "boom"), "MigrationSplit"),
        (ZingolibError.Migration(message: "boom"), "Migration"),
        (ZingolibError.Mixnet(message: "boom"), "Mixnet"),
    ]

    func testResolvedValuesPassThroughUnclassified() {
        // The value deliberately wears the historical error sentinel:
        // classification must be by channel, never by content.
        let proseLikeData = "Error: looks like prose but is legitimate data"

        guard case .resolved(let value) = FfiOutcome.of({ proseLikeData }) else {
            return XCTFail("A returning call must resolve")
        }
        XCTAssertEqual(value, proseLikeData, "A returning call must resolve its value verbatim")
    }

    func testThrownFfiErrorsRejectUnderTheVariantName() {
        for (failure, expectedCode) in contractVariants {
            guard case .rejected(let code, let message, let error) = FfiOutcome.of({ throw failure }) else {
                return XCTFail("Variant \(expectedCode) must reject on a thrown error")
            }
            XCTAssertEqual(code, expectedCode, "The rejection code is exactly the variant's name")
            XCTAssertEqual(message, "boom", "The rejection message is the error's message, verbatim")
            XCTAssertTrue(error is ZingolibError, "Variant \(expectedCode) must reject with its typed error")
        }
    }

    func testNonFfiErrorsRejectAsUnknown() {
        struct Boom: Error {}
        guard case .rejected(let code, let message, let error) = FfiOutcome.of({ throw Boom() }) else {
            return XCTFail("A non-FFI error must still reject")
        }
        XCTAssertEqual(code, "Unknown", "Errors outside the contract reject under the catch-all code")
        XCTAssertFalse(message.isEmpty, "Even a catch-all rejection carries a diagnostic message")
        XCTAssertTrue(error is Boom, "The original error object crosses the bridge")
    }
}

/// The numeric-arg contract of the bridge (zingo-mobile#1151): a malformed
/// or overflowing string throws the typed InvalidInput with the same
/// message shape the Android bridge rejects with — never a silent default
/// (the old per_bucket bug) and never an unsettled promise (the old
/// reschedule/execute bug). The Swift twin of the Kotlin FfiArgsTest.
class FfiArgsTests: XCTestCase {
    func testValidNumbersParse() throws {
        XCTAssertEqual(try FfiArgs.requiredU32("7", name: "per_bucket"), 7)
        XCTAssertEqual(try FfiArgs.requiredU32("4294967295", name: "per_bucket"), UInt32.max)
        XCTAssertEqual(try FfiArgs.requiredU64("250", name: "spacing_ms"), 250)
        XCTAssertEqual(
            try FfiArgs.requiredU64("18446744073709551615", name: "spacing_ms"), UInt64.max)
        XCTAssertEqual(try FfiArgs.optionalU32("7", name: "per_bucket"), 7)
    }

    func testEmptyOptionalMeansAbsentNeverZero() throws {
        XCTAssertNil(try FfiArgs.optionalU32("", name: "per_bucket"))
    }

    func testMalformedAndOverflowingValuesRejectAsInvalidInput() {
        let rejected: [(raw: String, parse: () throws -> Any)] = [
            ("not-a-number", { try FfiArgs.requiredU32("not-a-number", name: "per_bucket") }),
            ("-1", { try FfiArgs.requiredU32("-1", name: "per_bucket") }),
            ("4294967296", { try FfiArgs.requiredU32("4294967296", name: "per_bucket") }),
            ("1.5", { try FfiArgs.optionalU32("1.5", name: "per_bucket") as Any }),
            ("18446744073709551616",
             { try FfiArgs.requiredU64("18446744073709551616", name: "spacing_ms") }),
        ]
        for (raw, parse) in rejected {
            XCTAssertThrowsError(try parse(), "\"\(raw)\" must reject, never default") { error in
                guard case ZingolibError.InvalidInput = error else {
                    return XCTFail("\"\(raw)\" must throw the typed InvalidInput, got \(error)")
                }
            }
        }
    }

    func testTheRejectionMessageMatchesTheAndroidBridgeShape() {
        XCTAssertThrowsError(try FfiArgs.requiredU32("nope", name: "per_bucket")) { error in
            guard case ZingolibError.InvalidInput(let message) = error else {
                return XCTFail("expected the typed InvalidInput, got \(error)")
            }
            XCTAssertEqual(message, "per_bucket must be a u32: \"nope\"")
        }
        XCTAssertThrowsError(try FfiArgs.requiredU64("nope", name: "spacing_ms")) { error in
            guard case ZingolibError.InvalidInput(let message) = error else {
                return XCTFail("expected the typed InvalidInput, got \(error)")
            }
            XCTAssertEqual(message, "spacing_ms must be a u64: \"nope\"")
        }
    }

    func testTheRejectionCrossesTheBridgeAsInvalidInputNeverUnknown() {
        let outcome = FfiOutcome.of {
            _ = try FfiArgs.requiredU32("not-a-number", name: "per_bucket")
            return ""
        }
        guard case .rejected(let code, _, _) = outcome else {
            return XCTFail("a malformed numeric arg must reject")
        }
        XCTAssertEqual(
            code, "InvalidInput",
            "a malformed numeric arg must reject under InvalidInput on both platforms")
    }
}

/// The startup attribute migration: wallet files an old build wrote under
/// class A move to class C with backup exclusion, content untouched.
class WalletFileProtectionTests: XCTestCase {
    func testClassAFileMovesToClassCWithBackupExclusion() throws {
        let rpc = RPCModule()
        let fm = FileManager.default
        try fm.createDirectory(
            atPath: rpc.getDocumentsDirectory(),
            withIntermediateDirectories: true
        )
        let path = try rpc.getFileName(Constants.WalletFileName.rawValue)
        try "d2FsbGV0".write(toFile: path, atomically: true, encoding: .utf8)
        defer { try? fm.removeItem(atPath: path) }
        try fm.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: path)
        let stored = try fm.attributesOfItem(atPath: path)[.protectionKey] as? FileProtectionType

        rpc.applyWalletFileProtection()

        XCTAssertEqual(try String(contentsOfFile: path, encoding: .utf8), "d2FsbGV0")
        let excluded = try URL(fileURLWithPath: path)
            .resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup
        XCTAssertEqual(excluded, true)
        guard stored == .complete else {
            throw XCTSkip("this simulator does not store file-protection attributes")
        }
        let after = try fm.attributesOfItem(atPath: path)[.protectionKey] as? FileProtectionType
        XCTAssertEqual(after, .completeUntilFirstUserAuthentication)
    }

    func testMissingWalletFilesAreANoOp() throws {
        let rpc = RPCModule()
        let fm = FileManager.default
        for name in [Constants.WalletFileName.rawValue, Constants.WalletBackupFileName.rawValue] {
            if let path = try? rpc.getFileName(name) {
                try? fm.removeItem(atPath: path)
            }
        }
        rpc.applyWalletFileProtection()
    }
}

/// The per-file diagnosis behind the recovery dialog, and the migration
/// from the legacy base64 text format to raw wallet bytes.
class WalletFileDiagnosisTests: XCTestCase {
    private func mainEntry(_ rpc: RPCModule) -> [String: Any]? {
        rpc.walletFileDiagnosis().first { $0["name"] as? String == Constants.WalletFileName.rawValue }
    }

    private func mainPath(_ rpc: RPCModule) throws -> String {
        try FileManager.default.createDirectory(
            atPath: rpc.getDocumentsDirectory(),
            withIntermediateDirectories: true
        )
        return try rpc.getFileName(Constants.WalletFileName.rawValue)
    }

    override func tearDown() {
        let rpc = RPCModule()
        if let path = try? rpc.getFileName(Constants.WalletFileName.rawValue) {
            try? FileManager.default.removeItem(atPath: path)
        }
        super.tearDown()
    }

    private func walletBytes() -> Data {
        var wallet = Data([42, 0, 0, 0, 0, 0, 0, 0])
        wallet.append(Data(repeating: 7, count: 64))
        return wallet
    }

    func testARawWalletFileDiagnosesPlainWallet() throws {
        let rpc = RPCModule()
        let path = try mainPath(rpc)
        try walletBytes().write(to: URL(fileURLWithPath: path))

        let entry = try XCTUnwrap(mainEntry(rpc))
        XCTAssertEqual(entry["state"] as? String, "plainWallet")
        XCTAssertGreaterThan(entry["size"] as? Int ?? 0, 0)
    }

    func testATruncatedRawWalletFileDiagnosesPlainWallet() throws {
        let rpc = RPCModule()
        let path = try mainPath(rpc)
        try walletBytes().prefix(20).write(to: URL(fileURLWithPath: path))

        let entry = try XCTUnwrap(mainEntry(rpc))
        XCTAssertEqual(entry["state"] as? String, "plainWallet")
    }

    func testALegacyBase64TextFileDiagnosesPlainWallet() throws {
        let rpc = RPCModule()
        let path = try mainPath(rpc)
        let text = walletBytes().base64EncodedString()
        try String(text.prefix(text.count / 2 + 1)).write(
            toFile: path, atomically: true, encoding: .utf8)

        let entry = try XCTUnwrap(mainEntry(rpc))
        XCTAssertEqual(entry["state"] as? String, "plainWallet")
    }

    func testGarbageTextDiagnosesUnknown() throws {
        let rpc = RPCModule()
        let path = try mainPath(rpc)
        try "!!!not-base64!!!".write(toFile: path, atomically: true, encoding: .utf8)

        let entry = try XCTUnwrap(mainEntry(rpc))
        XCTAssertEqual(entry["state"] as? String, "unknown")
    }

    func testALegacyTextFileMigratesToRawBytesOnRead() throws {
        try requireLiveRegtest()
        try setCryptoProvider()
        let rpc = RPCModule()
        let walletDirectory = try selectWalletDirectory()
        defer { try? FileManager.default.removeItem(at: walletDirectory) }
        _ = try initFromSeed(
            seed: Seeds.HOSPITAL, birthday: UInt32(1), serveruri: WcashRegtest.endpoint,
            chainhint: WcashRegtest.chainHint, performancelevel: "Medium",
            minconfirmations: UInt32(1))
        let wallet = try XCTUnwrap(try saveWalletBytes())

        let path = try mainPath(rpc)
        try wallet.base64EncodedString().write(
            toFile: path, atomically: true, encoding: .utf8)

        XCTAssertEqual(try rpc.readWalletBytes(), wallet)
        XCTAssertEqual(try Data(contentsOf: URL(fileURLWithPath: path)), wallet)
    }

    func testAnInvalidLegacyTextFileFailsAndLeavesTheFileUntouched() throws {
        let rpc = RPCModule()
        let path = try mainPath(rpc)
        let text = walletBytes().base64EncodedString()
        try text.write(toFile: path, atomically: true, encoding: .utf8)

        XCTAssertThrowsError(try rpc.readWalletBytes())
        XCTAssertEqual(
            try Data(contentsOf: URL(fileURLWithPath: path)),
            text.data(using: .utf8))
    }

    func testAMissingFileDiagnosesMissing() throws {
        let rpc = RPCModule()
        let path = try mainPath(rpc)
        try? FileManager.default.removeItem(atPath: path)

        let entry = try XCTUnwrap(mainEntry(rpc))
        XCTAssertEqual(entry["state"] as? String, "missing")
    }
}

/// The wallet and backup swap recovered from every interruption window,
/// and the delete purge of sidecar copies.
class WalletSwapRecoveryTests: XCTestCase {
    let walletA = "walletA"
    let walletB = "walletB"
    let walletC = "walletC"

    override func tearDown() {
        let rpc = RPCModule()
        let fm = FileManager.default
        for name in [Constants.WalletFileName.rawValue,
                     Constants.WalletBackupFileName.rawValue,
                     Constants.WalletTempSwapFileName.rawValue,
                     "\(Constants.WalletFileName.rawValue).broken"] {
            if let path = try? rpc.getFileName(name) {
                try? fm.removeItem(atPath: path)
            }
        }
        RPCModule.walletFileClosed = false
        super.tearDown()
    }

    func testAClosedWalletFileRefusesTheSave() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        try walletA.write(toFile: files.main, atomically: true, encoding: .utf8)

        RPCModule.walletFileClosed = true
        try rpc.saveWalletInternal()

        XCTAssertEqual(try read(files.main), walletA)
    }

    func testDeleteClosesTheWalletFile() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        RPCModule.walletFileClosed = false
        try walletA.write(toFile: files.main, atomically: true, encoding: .utf8)

        try rpc.fnDeleteExistingWallet()

        XCTAssertTrue(RPCModule.walletFileClosed)
    }

    private func paths(_ rpc: RPCModule) throws -> (main: String, backup: String, temp: String) {
        try FileManager.default.createDirectory(
            atPath: rpc.getDocumentsDirectory(),
            withIntermediateDirectories: true
        )
        return (
            try rpc.getFileName(Constants.WalletFileName.rawValue),
            try rpc.getFileName(Constants.WalletBackupFileName.rawValue),
            try rpc.getFileName(Constants.WalletTempSwapFileName.rawValue)
        )
    }

    private func clear(_ files: (main: String, backup: String, temp: String)) {
        let fm = FileManager.default
        for path in [files.main, files.backup, files.temp] {
            try? fm.removeItem(atPath: path)
        }
    }

    private func read(_ path: String) throws -> String {
        try String(contentsOfFile: path, encoding: .utf8)
    }

    func testInterruptedBeforeMainRenameFinishesTheSwap() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        try walletA.write(toFile: files.temp, atomically: true, encoding: .utf8)
        try walletB.write(toFile: files.backup, atomically: true, encoding: .utf8)

        rpc.completePendingSwap()

        XCTAssertEqual(try read(files.main), walletB)
        XCTAssertEqual(try read(files.backup), walletA)
        XCTAssertFalse(FileManager.default.fileExists(atPath: files.temp))
        clear(files)
    }

    func testInterruptedBeforeBackupRenameFinishesTheSwap() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        try walletA.write(toFile: files.temp, atomically: true, encoding: .utf8)
        try walletB.write(toFile: files.main, atomically: true, encoding: .utf8)

        rpc.completePendingSwap()

        XCTAssertEqual(try read(files.main), walletB)
        XCTAssertEqual(try read(files.backup), walletA)
        XCTAssertFalse(FileManager.default.fileExists(atPath: files.temp))
        clear(files)
    }

    func testASaveRecreatingMainFinishesTheSwap() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        try walletA.write(toFile: files.temp, atomically: true, encoding: .utf8)
        try walletA.write(toFile: files.main, atomically: true, encoding: .utf8)
        try walletB.write(toFile: files.backup, atomically: true, encoding: .utf8)

        rpc.completePendingSwap()

        XCTAssertEqual(try read(files.main), walletB)
        XCTAssertEqual(try read(files.backup), walletA)
        XCTAssertFalse(FileManager.default.fileExists(atPath: files.temp))
        clear(files)
    }

    func testACompletedSwapDropsTheTemp() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        try walletA.write(toFile: files.temp, atomically: true, encoding: .utf8)
        try walletB.write(toFile: files.main, atomically: true, encoding: .utf8)
        try walletA.write(toFile: files.backup, atomically: true, encoding: .utf8)

        rpc.completePendingSwap()

        XCTAssertEqual(try read(files.main), walletB)
        XCTAssertEqual(try read(files.backup), walletA)
        XCTAssertFalse(FileManager.default.fileExists(atPath: files.temp))
        clear(files)
    }

    func testThreeDistinctWalletFilesAreLeftUntouched() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        try walletA.write(toFile: files.temp, atomically: true, encoding: .utf8)
        try walletC.write(toFile: files.main, atomically: true, encoding: .utf8)
        try walletB.write(toFile: files.backup, atomically: true, encoding: .utf8)

        rpc.completePendingSwap()

        XCTAssertEqual(try read(files.main), walletC)
        XCTAssertEqual(try read(files.backup), walletB)
        XCTAssertEqual(try read(files.temp), walletA)
    }

    func testDeleteKeepsAnUnresolvedSwapTemp() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        try walletA.write(toFile: files.temp, atomically: true, encoding: .utf8)
        try walletC.write(toFile: files.main, atomically: true, encoding: .utf8)
        try walletB.write(toFile: files.backup, atomically: true, encoding: .utf8)

        try rpc.fnDeleteExistingWallet()

        XCTAssertFalse(FileManager.default.fileExists(atPath: files.main))
        XCTAssertEqual(try read(files.temp), walletA)
        XCTAssertEqual(try read(files.backup), walletB)
    }

    func testDeleteRemovesTheBrokenCopyAndTheSwapTemp() throws {
        let rpc = RPCModule()
        let files = try paths(rpc)
        clear(files)
        let brokenPath = try rpc.getFileName("\(Constants.WalletFileName.rawValue).broken")
        try? FileManager.default.removeItem(atPath: brokenPath)
        try walletA.write(toFile: files.main, atomically: true, encoding: .utf8)
        try walletA.write(toFile: brokenPath, atomically: true, encoding: .utf8)
        try walletA.write(toFile: files.temp, atomically: true, encoding: .utf8)

        try rpc.fnDeleteExistingWallet()

        let fm = FileManager.default
        XCTAssertFalse(fm.fileExists(atPath: files.main))
        XCTAssertFalse(fm.fileExists(atPath: brokenPath))
        XCTAssertFalse(fm.fileExists(atPath: files.temp))
        clear(files)
    }
}
