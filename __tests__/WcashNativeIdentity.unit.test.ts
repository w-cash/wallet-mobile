import { readFileSync } from 'node:fs';

const readRepoFile = (path: string): string =>
  readFileSync(`${process.cwd()}/${path}`, 'utf8');

test('Tests that native data isolation holds when the mobile projects build.', () => {
  const androidGradle = readRepoFile('android/app/build.gradle.kts');
  const androidManifest = readRepoFile(
    'android/app/src/main/AndroidManifest.xml',
  );
  const iosAppDelegate = readRepoFile('ios/AppDelegate.swift');
  const iosInfo = readRepoFile('ios/Zingo/Info.plist');
  const iosProject = readRepoFile('ios/Zingo.xcodeproj/project.pbxproj');

  expect(
    androidGradle.match(/applicationId = "com\.wcashwallet\.warden\.testnet"/g),
  ).toHaveLength(1);
  expect(
    iosProject.match(
      /PRODUCT_BUNDLE_IDENTIFIER = com\.wcashwallet\.warden\.testnet;/g,
    ),
  ).toHaveLength(2);
  expect(
    iosProject.match(
      /PRODUCT_BUNDLE_IDENTIFIER = com\.wcashwallet\.warden\.testnet\.beta;/g,
    ),
  ).toHaveLength(2);
  expect(androidManifest).not.toContain('android:scheme="zcash"');
  expect(iosInfo).not.toContain('<string>zcash</string>');
  expect(iosInfo).toContain('com.wcashwallet.warden.testnet.processing');
  expect(iosAppDelegate).toContain('com.wcashwallet.warden.testnet.processing');
});
