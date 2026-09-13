#!/usr/bin/env node
// Build the iOS Wcash Wallet XCFramework: contains both the device slice (arm64) and
// the simulator slice (arm64 + x86_64 fat). Xcode auto-selects the right slice
// per build destination, so there is no separate "for device" vs "for simulator"
// build anymore.
//
// Output:
//   <repo>/ios/Zingolib.xcframework/  (the bundle Xcode links against)
//   <repo>/ios/zingo.swift            (Swift bindings, compiled as part of the app)
//
// macOS only.

import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, rmSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

if (process.platform !== 'darwin') {
  console.error('ERROR: iOS builds require macOS with Xcode installed.');
  process.exit(1);
}

const IOS_DIR = dirname(fileURLToPath(import.meta.url));
const RUST_DIR = resolve(IOS_DIR, '..');
const WCASH_FFI_DIR = join(RUST_DIR, 'wcash-mobile-ffi');
const TARGET_DIR = join(RUST_DIR, 'target');
const REPO_IOS_DIR = resolve(RUST_DIR, '..', 'ios');

const DEVICE_TARGET = 'aarch64-apple-ios';
const SIM_TARGETS = ['aarch64-apple-ios-sim', 'x86_64-apple-ios'];

const SIM_FAT_DIR = join(TARGET_DIR, 'universal-sim', 'release');
const SIM_FAT_LIB = join(SIM_FAT_DIR, 'libzingo.a');
const DEVICE_LIB = join(TARGET_DIR, DEVICE_TARGET, 'release', 'libzingo.a');
const XCF_HEADERS_DIR = join(TARGET_DIR, 'xcframework-headers');
const XCFRAMEWORK_OUT = join(REPO_IOS_DIR, 'Zingolib.xcframework');

function run(cmd, args, opts = {}) {
  console.log(`$ ${cmd} ${args.join(' ')}`);
  const r = spawnSync(cmd, args, { stdio: 'inherit', ...opts });
  if (r.status !== 0) {
    console.error(`ERROR: ${cmd} failed (exit ${r.status})`);
    process.exit(r.status ?? 1);
  }
}

function capture(cmd, args) {
  const r = spawnSync(cmd, args, { encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim() : null;
}

const env = { ...process.env, IPHONEOS_DEPLOYMENT_TARGET: '16.0' };

// 0. Clean up legacy artifacts from the pre-xcframework build flow. Idempotent:
//    after the first run on a clean checkout these files are gone forever.
for (const stale of [
  'libuniffi_zingo.a',
  'zingoFFI.h',
  'zingoFFI.modulemap',
  'zingo_nym_proxy_ffi.swift',
  'ZingoNymProxyFFI.xcframework',
]) {
  rmSync(join(REPO_IOS_DIR, stale), { force: true });
}

if (!capture('bindgen', ['--version'])) {
  run('cargo', ['install', '--force', '--locked', 'bindgen-cli'], { env });
}

// 1. Generate UniFFI Swift bindings from the Wcash-only UDL (also produces the
//    C header + modulemap). Generated output never touches upstream rust/lib.
const generated = join(WCASH_FFI_DIR, 'Generated');
rmSync(generated, { recursive: true, force: true });
mkdirSync(generated, { recursive: true });
process.chdir(RUST_DIR);
run(
  'cargo',
  [
    'run',
    '--locked',
    '--release',
    '--package',
    'zingo-uniffi-bindgen',
    '--bin',
    'zingo-wallet-uniffi-bindgen',
    '--',
    'generate',
    'wcash-mobile-ffi/src/zingo.udl',
    '--language',
    'swift',
    '--out-dir',
    generated,
  ],
  { env },
);

// 2. Build cargo for the 3 targets
for (const target of [DEVICE_TARGET, ...SIM_TARGETS]) {
  run(
    'cargo',
    [
      'build',
      '--locked',
      '--release',
      '--target',
      target,
      '--package',
      'wcash-mobile-ffi',
    ],
    {
      env,
      cwd: WCASH_FFI_DIR,
    },
  );
}

// 3. Lipo the 2 simulator targets into one fat .a
mkdirSync(SIM_FAT_DIR, { recursive: true });
run('lipo', [
  '-create',
  join(TARGET_DIR, 'aarch64-apple-ios-sim', 'release', 'libzingo.a'),
  join(TARGET_DIR, 'x86_64-apple-ios', 'release', 'libzingo.a'),
  '-output',
  SIM_FAT_LIB,
]);

// 4. Build the Wcash-only header directory and XCFramework.
rmSync(XCF_HEADERS_DIR, { recursive: true, force: true });
mkdirSync(XCF_HEADERS_DIR, { recursive: true });
copyFileSync(
  join(generated, 'zingoFFI.h'),
  join(XCF_HEADERS_DIR, 'zingoFFI.h'),
);
copyFileSync(
  join(generated, 'zingoFFI.modulemap'),
  join(XCF_HEADERS_DIR, 'module.modulemap'),
);

if (existsSync(XCFRAMEWORK_OUT)) {
  rmSync(XCFRAMEWORK_OUT, { recursive: true, force: true });
}
run('xcodebuild', [
  '-create-xcframework',
  '-library',
  DEVICE_LIB,
  '-headers',
  XCF_HEADERS_DIR,
  '-library',
  SIM_FAT_LIB,
  '-headers',
  XCF_HEADERS_DIR,
  '-output',
  XCFRAMEWORK_OUT,
]);

copyFileSync(join(generated, 'zingo.swift'), join(REPO_IOS_DIR, 'zingo.swift'));

console.log(`\nDone. Wcash wallet XCFramework at ${XCFRAMEWORK_OUT}`);
