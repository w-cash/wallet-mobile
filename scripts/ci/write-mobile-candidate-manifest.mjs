#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';
const [rootArg, network, version, build, sourceSha, repository] =
  process.argv.slice(2);

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

if (!rootArg) fail('an artifact directory is required');
if (network !== 'mainnet') fail(`unsupported candidate network ${network}`);
if (!/^\d+\.\d+\.\d+$/.test(version ?? ''))
  fail('the candidate version is invalid');
if (!/^\d+$/.test(build ?? '')) fail('the candidate build number is invalid');
if (!/^[0-9a-f]{40}$/.test(sourceSha ?? ''))
  fail('the source revision must be a full Git SHA');

const root = resolve(rootArg);

function listFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? listFiles(path) : [path];
  });
}

const files = listFiles(root)
  .filter(path => path.endsWith('.apk') || path.endsWith('.zip'))
  .sort();
const apkFiles = files.filter(path =>
  path.endsWith('-android-arm64-prodDebug.apk'),
);
const simulatorFiles = files.filter(path =>
  path.endsWith('-ios-arm64-simulator-unsigned.zip'),
);
const deviceFiles = files.filter(path =>
  path.endsWith('-ios-arm64-device-compile-unsigned.zip'),
);
if (
  files.length !== 3 ||
  apkFiles.length !== 1 ||
  simulatorFiles.length !== 1 ||
  deviceFiles.length !== 1
) {
  fail('the candidate must contain one Android APK and two iOS ZIPs');
}

const expectedStem = `Wcash-Wallet-${network}-${version}-${build}-${sourceSha.slice(0, 12)}`;
if (files.some(path => !basename(path).startsWith(expectedStem))) {
  fail('an artifact filename does not match the candidate metadata');
}

const artifacts = files.map(path => {
  const bytes = readFileSync(path);
  const name = basename(path);
  const platform = name.endsWith('-ios-arm64-simulator-unsigned.zip')
    ? 'ios-arm64-simulator'
    : name.endsWith('-ios-arm64-device-compile-unsigned.zip')
      ? 'ios-arm64-device-compile'
      : 'android-arm64-prodDebug';
  return {
    file: name,
    platform,
    signing: platform.startsWith('ios-')
      ? 'disabled'
      : 'Android debug certificate',
    bytes: statSync(path).size,
    sha256: createHash('sha256').update(bytes).digest('hex'),
  };
});

const manifest = {
  product: 'Wcash Wallet',
  network,
  version,
  build: Number(build),
  repository,
  sourceSha,
  rust: '1.91.0',
  xcode: '26.6',
  distribution: 'QA candidate',
  artifacts,
};

writeFileSync(
  join(root, 'MANIFEST.json'),
  `${JSON.stringify(manifest, undefined, 2)}\n`,
);
writeFileSync(
  join(root, 'SHA256SUMS'),
  `${artifacts.map(artifact => `${artifact.sha256}  ${artifact.file}`).join('\n')}\n`,
);

console.log(`wrote candidate metadata for ${artifacts.length} artifacts`);
