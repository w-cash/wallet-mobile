#!/usr/bin/env node
// Bumps version + build number for a release channel.
//
// Usage:
//   node scripts/release/release-prep.mjs <channel> <version> <build>
//   yarn release:prod:prep <version> <build>
//   yarn release:beta:prep <version> <build>
//
//   channel : "prod" | "beta"
//   version : semver string, e.g. 2.0.19
//   build   : integer, e.g. 308
//
// What it touches:
//
//   prod:
//     - package.json                        version
//     - ios/Zingo.xcodeproj/project.pbxproj target Zingo  Debug + Release
//                                           (MARKETING_VERSION, CURRENT_PROJECT_VERSION)
//     - android/app/build.gradle.kts        defaultConfig
//                                           (versionName, versionCode)
//
//   beta:
//     - ios/Zingo.xcodeproj/project.pbxproj target Zingo  Debug-Beta + Release-Beta
//                                           (MARKETING_VERSION, CURRENT_PROJECT_VERSION)
//     - android/app/build.gradle.kts        productFlavors.beta
//                                           (versionName, versionCode)
//
// Idempotent: running with the values already on disk is a no-op.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, relative } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = dirname(dirname(__dirname));

const [, , channel, version, build] = process.argv;

function die(msg) {
  console.error(`error: ${msg}`);
  console.error('usage: release-prep <prod|beta> <version> <build>');
  console.error('  e.g. release-prep beta 2.0.19 308');
  process.exit(1);
}

if (!['prod', 'beta'].includes(channel))
  die(`channel must be prod or beta (got "${channel}")`);
if (!/^\d+\.\d+\.\d+$/.test(version || ''))
  die(`version must be semver MAJOR.MINOR.PATCH (got "${version}")`);
if (!/^\d+$/.test(build || ''))
  die(`build must be an integer (got "${build}")`);
if (Number(build) > 9999) die(`build must be 9999 or lower (got "${build}")`);

const PACKAGE_JSON = join(repoRoot, 'package.json');
const PBXPROJ = join(repoRoot, 'ios/Zingo.xcodeproj/project.pbxproj');
const GRADLE = join(repoRoot, 'android/app/build.gradle.kts');

function patch(filePath, transform) {
  const before = readFileSync(filePath, 'utf8');
  const after = transform(before);
  const rel = relative(repoRoot, filePath);
  if (before === after) {
    console.log(`  ~ ${rel} (no change)`);
    return false;
  }
  writeFileSync(filePath, after);
  console.log(`  ✓ ${rel}`);
  return true;
}

// In pbxproj, each XCBuildConfiguration block is alphabetically sorted, so
// ASSETCATALOG_COMPILER_APPICON_NAME sits well above MARKETING_VERSION and
// CURRENT_PROJECT_VERSION within the same config. We anchor on it to scope
// the replacement to either prod (AppIcon) or beta ("AppIcon-Beta").
function patchPbxprojForChannel(src, appIconAnchor, version, build) {
  return src
    .replace(
      new RegExp(
        `(ASSETCATALOG_COMPILER_APPICON_NAME = ${appIconAnchor};[\\s\\S]*?)MARKETING_VERSION = [^;]+;`,
        'g',
      ),
      `$1MARKETING_VERSION = ${version};`,
    )
    .replace(
      new RegExp(
        `(ASSETCATALOG_COMPILER_APPICON_NAME = ${appIconAnchor};[\\s\\S]*?)CURRENT_PROJECT_VERSION = \\d+;`,
        'g',
      ),
      `$1CURRENT_PROJECT_VERSION = ${build};`,
    );
}

console.log(
  `release-prep: channel=${channel} version=${version} build=${build}`,
);

if (channel === 'prod') {
  patch(PACKAGE_JSON, src => {
    const pkg = JSON.parse(src);
    pkg.version = version;
    return JSON.stringify(pkg, null, 2) + '\n';
  });

  // iOS prod = target Zingo configs with "AppIcon-Prod".
  patch(PBXPROJ, src =>
    patchPbxprojForChannel(src, '"AppIcon-Prod"', version, build),
  );

  // Android prod = defaultConfig.
  patch(GRADLE, src =>
    src
      .replace(
        /versionCode = \d+ \/\/ Real.*$/m,
        `versionCode = ${build} // Real (prod baseline; beta flavor overrides below)`,
      )
      .replace(
        /versionName = "[^"]+" \/\/ Real$/m,
        `versionName = "${version}" // Real`,
      ),
  );
}

if (channel === 'beta') {
  // iOS beta = target Zingo configs with "AppIcon-Beta".
  patch(PBXPROJ, src =>
    patchPbxprojForChannel(src, '"AppIcon-Beta"', version, build),
  );

  // Android beta = productFlavors.beta, which overrides both versionCode and
  // versionName from defaultConfig so beta can ship a distinct semver from prod.
  patch(GRADLE, src =>
    src
      .replace(/(create\("beta"\) \{[\s\S]*?versionCode = )\d+/, `$1${build}`)
      .replace(
        /(create\("beta"\) \{[\s\S]*?versionName = )"[^"]+"/,
        `$1"${version}"`,
      ),
  );
}

const tag =
  channel === 'beta'
    ? `wcash-beta-${version}-${build}`
    : `wcash-${version}-${build}`;
const label = channel === 'beta' ? 'Wcash beta' : 'Wcash';

console.log('');
console.log('Ready. Suggested next steps:');
console.log('  git diff');
console.log('  git add -A');
console.log(`  git commit -m "release: ${label} ${version} (${build})"`);
console.log('  git push');
console.log(`  git tag -m "release ${label} ${version} (${build})" ${tag}`);
console.log(`  git push origin ${tag}`);
console.log('');
if (channel === 'prod') {
  console.log(
    'Run the Wcash mobile unsigned candidate workflow from GitHub Actions',
  );
  console.log('on this tag. The workflow produces a Local Regtest');
  console.log('ARM64 prodDebug APK, an unsigned ARM64 iOS simulator app, a');
  console.log('manifest, and SHA-256 checksums.');
} else {
  console.log('The unsigned candidate workflow does not accept beta tags.');
  console.log(
    'Add reviewed beta metadata, Android flavor, and iOS scheme inputs',
  );
  console.log('before producing a beta candidate.');
}
console.log('');
console.log(
  'The protected production signing workflow remains fail-closed until',
);
console.log('the signing implementation and store delivery are reviewed.');
console.log('');
console.log('done.');
