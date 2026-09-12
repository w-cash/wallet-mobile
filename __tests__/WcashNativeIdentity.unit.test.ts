import { readFileSync } from 'node:fs';

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
  const adapterSource = readRepoFile('rust/wcash-mobile-adapter/src/lib.rs');
  const ffiCargo = readRepoFile('rust/wcash-mobile-ffi/Cargo.toml');
  const ffiSource = readRepoFile('rust/wcash-mobile-ffi/src/lib.rs');
  const androidBridge = readRepoFile(
    'android/app/src/main/java/org/ZingoLabs/Zingo/RPCModule.kt',
  );
  const androidBuild = readRepoFile('rust/android/docker/Dockerfile');
  const androidLocalBuild = readRepoFile(
    'rust/android/build_android_local.mjs',
  );
  const kotlinBindingBuild = readRepoFile(
    'scripts/generate_kotlin_bindings.mjs',
  );
  const iosBridge = readRepoFile('ios/RPCModule.swift');
  const iosBuild = readRepoFile('rust/ios/build_ios.mjs');
  const wcashUdl = readRepoFile('rust/wcash-mobile-ffi/src/zingo.udl');
  const servers = readRepoFile('app/uris/serverUris.ts');
  const dataService = readRepoFile('app/walletBackend/modules/DataService.ts');
  const wcashQa = readRepoFile('.github/workflows/wcash-pr-qa.yaml');
  const upstreamCi = readRepoFile('.github/workflows/ci.yaml');
  const translations = ['en', 'es', 'pt', 'ru', 'tr'].flatMap(locale =>
    stringValues(
      JSON.parse(readRepoFile(`app/translations/${locale}.json`)) as unknown,
    ),
  );

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
    translations.filter(value => /zingo\s*labs?|zingo/i.test(value)),
  ).toEqual([]);
  expect(support).toContain('https://github.com/w-cash/wallet-mobile/issues');
  expect(support).not.toContain('mailto:');
  expect(cargo).toContain(
    'https://github.com/w-cash/wallet-core.git", rev = "58bc22ec63bbe3eddab5f961c137836431589c95',
  );
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
  expect(iosBridge).toContain('setWalletDirectory(directory:');
  expect(androidBuild.match(/--package wcash-mobile-ffi/g)).toHaveLength(4);
  expect(androidLocalBuild).toContain("'--package', 'wcash-mobile-ffi'");
  expect(kotlinBindingBuild).toContain(
    "'wcash-mobile-ffi', 'src', 'zingo.udl'",
  );
  expect(iosBuild).toContain("'--package', 'wcash-mobile-ffi'");
  expect(dataService).toContain(
    'unsupported Wcash feature: memo message history',
  );
  expect(servers).toContain("uri: 'http://127.0.0.1:48234'");
  expect(servers).toContain('chainName: ChainNameEnum.regtestChainName');
  expect(servers).not.toContain('zec.rocks');
  expect(servers).not.toContain('zcash-infra.com');
  expect(servers).not.toContain('lightwalletd.com');
  expect(wcashQa).toContain('runs-on: ubuntu-24.04');
  expect(wcashQa).toContain('runs-on: macos-15');
  expect(wcashQa.match(/--package wcash-mobile-ffi/g)).toHaveLength(2);
  expect(wcashQa).toContain('wcash-mobile-ffi/src/zingo.udl');
  expect(wcashQa).toContain('permissions:\n  contents: read');
  expect(wcashQa).not.toContain('softprops/action-gh-release');
  expect(wcashQa).not.toContain('pages deploy');
  expect(upstreamCi).not.toContain('  pull_request:');
});
