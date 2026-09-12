//
//  NymTransportModule.swift
//  Zingo
//
//  Wcash preserves the upstream React Native module surface but does not
//  bundle the upstream Nym shim. Start fails closed with a stable error;
//  stop remains an idempotent no-op for normal cleanup paths.
//

import Foundation
import React

@objc(NymTransportModule)
class NymTransportModule: NSObject {

  @objc
  static func requiresMainQueueSetup() -> Bool {
    return false
  }

  @objc(startMixnetTransport:reject:)
  func startMixnetTransport(_ resolve: @escaping RCTPromiseResolveBlock,
                            reject: @escaping RCTPromiseRejectBlock) {
    let message = "Wcash Wallet mixnet transport is unsupported by the reviewed Wcash backend."
    let error = NSError(
      domain: "start_mixnet_transport",
      code: 1,
      userInfo: [NSLocalizedDescriptionKey: message]
    )
    reject("start_mixnet_transport", message, error)
  }

  @objc(stopMixnetTransport:reject:)
  func stopMixnetTransport(_ resolve: @escaping RCTPromiseResolveBlock,
                           reject: @escaping RCTPromiseRejectBlock) {
    resolve(nil)
  }
}
