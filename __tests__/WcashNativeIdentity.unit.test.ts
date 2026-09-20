import { existsSync, readFileSync } from 'node:fs';

const readRepoFile = (path: string): string =>
  readFileSync(`${process.cwd()}/${path}`, 'utf8');

const stringValues = (value: unknown): string[] => {
  if (typeof value === 'string') {
    return [value];
  }
  if (Array.isArray(value)) {
    return value.flatMap(stringValues);
  }
  if (value && typeof value === 'object') {
    return Object.values(value).flatMap(stringValues);
  }
  return [];
};

test('Tests that the Wcash identity is isolated when the native apps build.', () => {
  const app = readRepoFile('app.json');
  const constants = readRepoFile('app/AppState/const/GlobalConst.ts');
  const android = readRepoFile('android/app/build.gradle.kts');
  const androidManifest = readRepoFile(
    'android/app/src/main/AndroidManifest.xml',
  );
  const iosInfo = readRepoFile('ios/Zingo/Info.plist');
  const iosProject = readRepoFile('ios/Zingo.xcodeproj/project.pbxproj');
  const iosLaunch = readRepoFile('ios/Zingo/LaunchScreen.storyboard');
  const currency = readRepoFile('app/AppState/enums/CurrencyNameEnum.ts');
  const support = readRepoFile('app/services/sendEmail.ts');
  const cargo = readRepoFile('rust/Cargo.toml');
  const cargoLock = readRepoFile('rust/Cargo.lock');
  const adapterBuild = readRepoFile('rust/wcash-mobile-adapter/build.rs');
  const adapterSource = readRepoFile('rust/wcash-mobile-adapter/src/lib.rs');
  const ffiCargo = readRepoFile('rust/wcash-mobile-ffi/Cargo.toml');
  const ffiSource = readRepoFile('rust/wcash-mobile-ffi/src/lib.rs');
  const androidBridge = readRepoFile(
    'android/app/src/main/java/org/ZingoLabs/Zingo/RPCModule.kt',
  );
  const androidDurableSave = readRepoFile(
    'android/app/src/main/java/org/ZingoLabs/Zingo/DurableWalletSave.kt',
  );
  const androidMixnetBridge = readRepoFile(
    'android/app/src/main/java/org/ZingoLabs/Zingo/NymTransportModule.kt',
  );
  const androidBuild = readRepoFile('rust/android/docker/Dockerfile');
  const androidLocalBuild = readRepoFile(
    'rust/android/build_android_local.mjs',
  );
  const kotlinBindingBuild = readRepoFile(
    'scripts/generate_kotlin_bindings.mjs',
  );
  const iosBridge = readRepoFile('ios/RPCModule.swift');
  const iosMixnetBridge = readRepoFile('ios/NymTransportModule.swift');
  const iosBuild = readRepoFile('rust/ios/build_ios.mjs');
  const wcashUdl = readRepoFile('rust/wcash-mobile-ffi/src/zingo.udl');
  const servers = readRepoFile('app/uris/serverUris.ts');
  const dataService = readRepoFile('app/walletBackend/modules/DataService.ts');
  const loadedApp = readRepoFile('app/LoadedApp/LoadedApp.tsx');
  const utils = readRepoFile('app/utils/Utils.ts');
  const wcashQa = readRepoFile('.github/workflows/wcash-pr-qa.yaml');
  const androidRelease = readRepoFile('.github/workflows/android-release.yaml');
  const androidReusableBuild = readRepoFile(
    '.github/workflows/android-build.yaml',
  );
  const androidApkWorkflow = readRepoFile(
    '.github/workflows/android-apk-build.yaml',
  );
  const androidIntegrationWorkflow = readRepoFile(
    '.github/workflows/android-ubuntu-integration-test-ci.yaml',
  );
  const iosWorkflow = readRepoFile('.github/workflows/ios-build.yaml');
  const iosIntegrationWorkflow = readRepoFile(
    '.github/workflows/ios-integration-test.yaml',
  );
  const mixnetWorkflow = readRepoFile(
    '.github/workflows/nym-proxy-ffi-check.yaml',
  );
  const upstreamCi = readRepoFile('.github/workflows/ci.yaml');
  const upstreamNightly = readRepoFile('.github/workflows/ci-nightly.yaml');
  const upstreamMaestro = readRepoFile(
    '.github/workflows/maestro-nightly.yaml',
  );
  const storybookWorkflow = readRepoFile(
    '.github/workflows/deploy-storybook.yaml',
  );
  const visualWorkflow = readRepoFile('.github/workflows/visual-review.yaml');
  const translationDocuments = ['en', 'es', 'pt', 'ru', 'tr'].map(locale =>
    JSON.parse(readRepoFile(`app/translations/${locale}.json`)),
  );
  const productTranslations = translationDocuments.flatMap(document => {
    const aboutProduct = { ...document.about };
    Reflect.deleteProperty(aboutProduct, 'copyright');
    return stringValues({ ...document, about: aboutProduct });
  });

  expect(JSON.parse(app)).toEqual({
    name: 'Zingo',
    displayName: 'Wcash Wallet',
  });
  expect(
    android.match(/applicationId = "com\.wcashwallet\.wallet"/g),
  ).toHaveLength(1);
  expect(android).toContain('applicationIdSuffix = ".beta"');
  expect(
    android.match(/resValue\("string", "app_name", "Wcash Wallet"\)/g),
  ).toHaveLength(2);
  expect(android).toContain('rejectLegacyNymInputs');
  expect(android).toContain('verifyNoLegacyNymPackageOutputs');
  expect(android).toContain('zingo_nym_proxy_ffi');
  expect(android).toContain('wcashRegtestQaArm64Only');
  expect(android).toContain('isEnable = splitApk || wcashRegtestQaArm64Only');
  expect(android).toContain('include("arm64-v8a")');
  expect(android).toContain('verifyWcashRegtestQaArm64Package');
  expect(android).not.toContain('Wcash Wallet Beta');
  expect(
    iosProject.match(/PRODUCT_BUNDLE_IDENTIFIER = com\.wcashwallet\.wallet;/g),
  ).toHaveLength(2);
  expect(
    iosProject.match(/BUNDLE_DISPLAY_NAME = "Wcash Wallet";/g),
  ).toHaveLength(4);
  expect(iosProject).not.toContain('Wcash Wallet Beta');
  expect(
    iosProject.match(
      /PRODUCT_BUNDLE_IDENTIFIER = com\.wcashwallet\.wallet\.beta;/g,
    ),
  ).toHaveLength(2);
  expect(constants).toContain("serviceKeyChain: 'WCASH_WALLET'");
  expect(constants).toContain("keyKeyChain: 'WCASH_WALLET_SEED_BIRTHDAY'");
  expect(constants).toContain("zcash: 'wcash:'");
  expect(androidManifest).toContain('android:scheme="wcash"');
  expect(androidManifest).not.toContain('android:scheme="zcash"');
  expect(iosInfo).toContain('<string>wcash</string>');
  expect(iosInfo).not.toContain('<string>zcash</string>');
  expect(iosInfo).toContain('Wcash Wallet needs access to the camera');
  expect(iosLaunch).toContain('text="Wcash Wallet"');
  expect(currency).toContain("ZEC = 'WEC'");
  expect(currency).toContain("TAZ = 'TWC'");
  expect(
    productTranslations.filter(value => /zingo\s*labs?|zingo/i.test(value)),
  ).toEqual([]);
  for (const document of translationDocuments) {
    expect(document.about.copyright[0]).toMatch(/Wcash Wallet/);
    expect(document.about.copyright[2]).toMatch(
      /Copyright \(c\) 2026 Wcash Wallet/,
    );
  }
  expect(constants).toContain("zingolib: 'Wcash Wallet'");
  expect(support).toContain('https://github.com/w-cash/wallet-mobile/issues');
  expect(support).not.toContain('mailto:');
  expect(cargo).toContain('https://github.com/w-cash/wallet-core.git');
  expect(cargo).not.toContain('https://github.com/zingolabs/zingolib.git');
  expect(cargo).toContain('exclude = ["lib", "nym-proxy-ffi"]');
  expect(cargo).not.toMatch(/features = \[[^\]]*"nym"/s);
  expect(cargoLock.match(/^name = "wcash-wallet"$/gm)).toHaveLength(1);
  expect(cargoLock).not.toContain('5b4e29980eb45e84ddab9024f530c923986d7e1e');
  const walletCoreRevision = cargo.match(
    /wallet-core\.git", rev = "([0-9a-f]{40})"/,
  )?.[1];
  expect(walletCoreRevision).toHaveLength(40);
  expect(cargoLock).toContain(
    `wallet-core.git?rev=${walletCoreRevision}#${walletCoreRevision}`,
  );
  expect(adapterBuild).toContain('WCASH_WALLET_CORE_REV');
  expect(adapterBuild).toContain('name = \\"zingolib\\"');
  expect(adapterSource).toContain(
    'pub const WCASH_WALLET_CORE_REV: &str = env!("WCASH_WALLET_CORE_REV")',
  );
  expect(adapterSource).toContain('"git_commit": WCASH_WALLET_CORE_REV');
  expect(ffiSource).toContain('wcash_mobile_adapter::WCASH_WALLET_CORE_REV');
  expect(adapterSource).toContain('WcashTestnetRuntime');
  expect(adapterSource).toContain('WcashRegtestRuntime');
  expect(adapterSource).not.toContain('zingolib::lightclient::LightClient');
  expect(ffiCargo).toContain('name = "wcash-mobile-ffi"');
  expect(ffiCargo).toContain('wcash-mobile-adapter');
  expect(ffiCargo).not.toContain('path = "../lib"');
  expect(ffiCargo).not.toContain('zingolib =');
  expect(ffiSource).not.toContain('zingolib::lightclient::LightClient');
  expect(ffiSource).toContain('unsupported Wcash feature');
  expect(wcashUdl).toContain('string set_wallet_directory(string directory)');
  expect(androidBridge).toContain('uniffi.zingo.setWalletDirectory');
  expect(androidBridge).toContain('applicationContext.noBackupFilesDir');
  expect(androidBridge).toContain(
    'Os.chmod(directory.absolutePath, OsConstants.S_IRWXU)',
  );
  expect(
    androidBridge.match(
      /requireDurableInitialWalletSave\(saveWalletFile\(\)\)/g,
    ),
  ).toHaveLength(3);
  expect(androidDurableSave).toContain(
    'Wcash Wallet initialization could not be saved durably.',
  );
  expect(androidMixnetBridge).toContain(
    'unsupported by the reviewed Wcash backend',
  );
  expect(androidMixnetBridge).not.toContain('uniffi.zingo_nym_proxy_ffi');
  expect(iosBridge).toContain('setWalletDirectory(directory:');
  expect(iosBridge).toContain('.applicationSupportDirectory');
  expect(iosBridge).toContain('"wcash-wallet.sqlite-wal"');
  expect(iosBridge).toContain('"wcash-wallet.sqlite-shm"');
  expect(iosBridge).toContain('isExcludedFromBackup = true');
  expect(iosBridge).toContain(
    'FileProtectionType.completeUntilFirstUserAuthentication',
  );
  expect(iosBridge).toContain('func protectWalletDatabaseFiles() throws');
  expect(iosBridge).not.toContain('try? url.setResourceValues(resourceValues)');
  expect(iosBridge).toContain(
    'fatalError("Wcash private database boundary failed:',
  );
  expect(iosBridge).toContain('func protectWalletFile(at fileURL: URL) throws');
  expect(iosBridge).toContain('.wcash-protected-write-');
  expect(iosBridge).toContain('try protectWalletFile(at: temporary)');
  expect(iosBridge).toContain('replaceItemAt(');
  expect(iosBridge).toContain('options: [.usingNewMetadataOnly]');
  expect(iosBridge).toContain(
    'try verifyWalletFileProtection(at: destination)',
  );
  expect(iosBridge).toContain('#if !targetEnvironment(simulator)');
  expect(iosBridge).toContain(
    'try? fm.moveItem(at: rollback, to: destination)',
  );
  expect(iosBridge).toContain('try? fm.removeItem(at: temporary)');
  expect(iosMixnetBridge).toContain(
    'unsupported by the reviewed Wcash backend',
  );
  expect(iosMixnetBridge).not.toContain('MixnetProxyHandle');
  expect(iosProject).not.toContain('ZingoNymProxyFFI.xcframework');
  expect(iosProject).not.toContain('zingo_nym_proxy_ffi.swift');
  expect(androidBuild.match(/--package wcash-mobile-ffi/g)).toHaveLength(4);
  expect(androidBuild).not.toContain('nym-proxy-ffi');
  expect(androidLocalBuild).toMatch(
    /['"]--package['"]\s*,\s*['"]wcash-mobile-ffi['"]/,
  );
  expect(androidLocalBuild).not.toContain('nym-proxy-ffi');
  expect(androidLocalBuild).toContain("'libzingo_nym_proxy_ffi.so'");
  expect(androidLocalBuild).toContain("'--regtest-qa-apk'");
  expect(androidLocalBuild).toContain("'-PwcashRegtestQaArm64Only=true'");
  expect(kotlinBindingBuild).toContain("'wcash-mobile-ffi'");
  expect(kotlinBindingBuild).toContain("'zingo.udl'");
  expect(kotlinBindingBuild).not.toContain('nym-proxy-ffi');
  expect(kotlinBindingBuild).toContain("'zingo_nym_proxy_ffi'");
  expect(iosBuild).toMatch(/['"]--package['"]\s*,\s*['"]wcash-mobile-ffi['"]/);
  expect(iosBuild).not.toContain('nym-proxy-ffi');
  expect(dataService).toContain(
    'unsupported Wcash feature: memo message history',
  );
  expect(loadedApp).toContain(
    'SettingsFileImpl.writeSettings(SettingsNameEnum.donation, value)',
  );
  expect(loadedApp).toContain('zenniesAddress &&');
  expect(utils).toContain('static async getDonationAddress');
  expect(utils).toContain("return '';");
  expect(servers).toContain('uri: WCASH_MAINNET_URI');
  expect(servers).toContain('chainName: ChainNameEnum.mainChainName');
  expect(servers).not.toContain('zec.rocks');
  expect(servers).not.toContain('zcash-infra.com');
  expect(servers).not.toContain('lightwalletd.com');
  expect(wcashQa).toContain('runs-on: ubuntu-24.04');
  expect(wcashQa).toContain('runs-on: macos-26');
  expect(wcashQa).toContain(
    'cargo clippy --locked -p wcash-mobile-adapter -p wcash-mobile-ffi --all-targets -- -D warnings',
  );
  expect(wcashQa).toContain(
    'cargo test --locked -p wcash-mobile-adapter -p wcash-mobile-ffi -p rustios',
  );
  expect(wcashQa).toContain('cargo tree --locked -p wcash-mobile-ffi');
  expect(wcashQa).toContain('permissions:\n  contents: read');
  expect(wcashQa).not.toContain('softprops/action-gh-release');
  expect(wcashQa).not.toContain('pages deploy');
  expect(androidRelease).toContain('Wcash mobile unsigned candidate');
  expect(androidRelease).toContain('contents: read');
  expect(
    androidRelease.match(
      /cargo tree --locked --manifest-path rust\/Cargo\.toml -p wcash-mobile-ffi/g,
    ),
  ).toHaveLength(2);
  expect(androidRelease).toContain('aarch64-linux-android');
  expect(androidRelease).toContain('aarch64-apple-ios-sim');
  expect(androidRelease).toContain('setWalletDirectory');
  expect(androidRelease).toContain('uniffi_zingo_fn_func_set_wallet_directory');
  expect(androidRelease).toContain("if grep -Eiq 'nym|mixnet'");
  expect(androidRelease).toContain('-sdk iphoneos');
  expect(androidRelease).toContain("-destination 'generic/platform=iOS'");
  expect(androidRelease).toContain('CODE_SIGNING_ALLOWED=NO');
  expect(androidRelease).toContain('ios-arm64-device-compile-unsigned');
  expect(androidRelease).toContain('actions/upload-artifact');
  expect(androidRelease).not.toMatch(
    /softprops\/action-gh-release|security import|store-password|key-password|pages deploy/i,
  );
  for (const disabledWorkflow of [
    androidReusableBuild,
    androidApkWorkflow,
    androidIntegrationWorkflow,
    iosWorkflow,
    iosIntegrationWorkflow,
    mixnetWorkflow,
  ]) {
    expect(disabledWorkflow).toContain('workflow_call:');
    expect(disabledWorkflow).toContain('contents: read');
    expect(disabledWorkflow).toContain('exit 1');
    expect(disabledWorkflow).not.toMatch(
      /upload-artifact|gradlew|build_ios|nym-proxy-ffi|working-directory: \.\/rust\/lib/,
    );
  }
  expect(upstreamCi).not.toContain('  pull_request:');
  expect(upstreamCi).toContain('contents: read');
  expect(upstreamCi).toContain('exit 1');
  expect(upstreamCi).not.toContain('actions: write');
  expect(upstreamNightly).not.toContain('schedule:');
  expect(upstreamNightly).toContain('exit 1');
  expect(upstreamMaestro).not.toContain('schedule:');
  expect(upstreamMaestro).toContain('exit 1');
  expect(storybookWorkflow).toContain('actions/upload-artifact');
  expect(storybookWorkflow).not.toMatch(/cloudflare|wrangler|pages deploy/i);
  expect(visualWorkflow).toContain('actions/upload-artifact');
  expect(visualWorkflow).not.toMatch(
    /cloudflare|wrangler|pages deploy|pull-requests: write|github-script/i,
  );
  expect(
    existsSync(`${process.cwd()}/.github/workflows/visual-accept.yaml`),
  ).toBe(false);
  expect(
    existsSync(`${process.cwd()}/.github/workflows/visual-review-cleanup.yaml`),
  ).toBe(false);
});
