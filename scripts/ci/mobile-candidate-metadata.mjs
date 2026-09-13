#!/usr/bin/env node

import { appendFileSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const network = process.argv[2];
const sourceSha = process.env.GITHUB_SHA ?? process.argv[3];

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

if (network !== 'regtest') fail(`unsupported candidate network ${network}`);
if (!/^[0-9a-f]{40}$/.test(sourceSha ?? ''))
  fail('the source revision must be a full Git SHA');

const pkg = JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8'));
const gradle = readFileSync(
  join(repoRoot, 'android/app/build.gradle.kts'),
  'utf8',
);
const pbx = readFileSync(
  join(repoRoot, 'ios/Zingo.xcodeproj/project.pbxproj'),
  'utf8',
);
const defaultConfig = gradle.match(
  /defaultConfig\s*\{([\s\S]*?)\n\s*flavorDimensions/,
)?.[1];
if (!defaultConfig) fail('could not read the Android production version');

const androidBuild = defaultConfig.match(/versionCode\s*=\s*(\d+)/)?.[1];
const androidVersion = defaultConfig.match(/versionName\s*=\s*"([^"]+)"/)?.[1];
if (!androidBuild || !androidVersion)
  fail('the Android production version is incomplete');

const prodConfigs = [...pbx.matchAll(/buildSettings = \{([\s\S]*?)\n\s*\};/g)]
  .map(match => match[1])
  .filter(settings =>
    settings.includes('ASSETCATALOG_COMPILER_APPICON_NAME = "AppIcon-Prod"'),
  );
if (prodConfigs.length !== 2)
  fail('the iOS project must have two production configurations');

const iosVersions = new Set(
  prodConfigs.map(
    settings => settings.match(/MARKETING_VERSION = ([^;]+);/)?.[1],
  ),
);
const iosBuilds = new Set(
  prodConfigs.map(
    settings => settings.match(/CURRENT_PROJECT_VERSION = (\d+);/)?.[1],
  ),
);
const iosIds = new Set(
  prodConfigs.map(
    settings => settings.match(/PRODUCT_BUNDLE_IDENTIFIER = ([^;]+);/)?.[1],
  ),
);
const iosNames = new Set(
  prodConfigs.map(
    settings => settings.match(/BUNDLE_DISPLAY_NAME = "([^"]+)";/)?.[1],
  ),
);

if (iosVersions.size !== 1 || !iosVersions.has(androidVersion))
  fail('the Android and iOS versions differ');
if (iosBuilds.size !== 1 || !iosBuilds.has(androidBuild))
  fail('the Android and iOS build numbers differ');
if (pkg.version !== androidVersion)
  fail('package.json and native production versions differ');
if (iosIds.size !== 1 || !iosIds.has('com.wcashwallet.wallet'))
  fail('the iOS production bundle ID is invalid');
if (iosNames.size !== 1 || !iosNames.has('Wcash Wallet'))
  fail('the iOS display name is invalid');
if (process.env.GITHUB_REF_TYPE === 'tag') {
  const expectedTag = `wcash-${androidVersion}-${androidBuild}`;
  if (process.env.GITHUB_REF_NAME !== expectedTag) {
    fail(`the production candidate tag must be ${expectedTag}`);
  }
}

const sha = sourceSha.slice(0, 12);
const stem = `Wcash-Wallet-${network}-${androidVersion}-${androidBuild}-${sha}`;
const fields = {
  network,
  version: androidVersion,
  build: androidBuild,
  source_sha: sourceSha,
  sha,
  stem,
};

if (process.env.GITHUB_OUTPUT) {
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    Object.entries(fields)
      .map(([key, field]) => `${key}=${field}\n`)
      .join(''),
  );
} else {
  console.log(JSON.stringify(fields, undefined, 2));
}
