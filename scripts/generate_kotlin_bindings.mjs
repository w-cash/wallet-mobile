#!/usr/bin/env node
// Generates only the Android Kotlin UniFFI binding for the reviewed Wcash
// wallet boundary. The output package stays `uniffi.zingo` for compatibility
// with the unchanged native bridge.
//
// Usage: node scripts/generate_kotlin_bindings.mjs
//          [--variants release|debug,release,...]  (default: release)

import { spawnSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_DIR = resolve(SCRIPTS_DIR, '..');
const RUST_DIR = join(REPO_DIR, 'rust');
const UDL = join(RUST_DIR, 'wcash-mobile-ffi', 'src', 'zingo.udl');
const OUT_ROOT = join(
  REPO_DIR,
  'android',
  'app',
  'build',
  'generated',
  'source',
  'uniffi',
);

const exe = process.platform === 'win32' ? '.exe' : '';
const WALLET_BINDGEN = join(
  RUST_DIR,
  'target',
  'release',
  `zingo-wallet-uniffi-bindgen${exe}`,
);

function parseArgs(argv) {
  let variants = ['release'];
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    if (flag === '--variants') {
      variants = argv[++i].split(',').filter(Boolean);
    } else if (flag.startsWith('--variants=')) {
      variants = flag.slice('--variants='.length).split(',').filter(Boolean);
    } else {
      console.error(`unknown flag: ${flag}`);
      process.exit(2);
    }
  }
  return variants;
}

function run(cmd, args, cwd) {
  const { status } = spawnSync(cmd, args, { cwd, stdio: 'inherit' });
  if (status !== 0) {
    console.error(`${cmd} ${args.join(' ')} failed (${status})`);
    process.exit(status ?? 1);
  }
}

const variants = parseArgs(process.argv.slice(2));

console.log('=== Building the Wcash wallet bindgen binary ===');
run(
  'cargo',
  [
    'build',
    '--release',
    '--locked',
    '--package',
    'zingo-uniffi-bindgen',
    '--bin',
    'zingo-wallet-uniffi-bindgen',
  ],
  RUST_DIR,
);

for (const variant of variants) {
  const outDir = join(OUT_ROOT, variant, 'java');
  rmSync(join(outDir, 'uniffi', 'zingo_nym_proxy_ffi'), {
    recursive: true,
    force: true,
  });
  mkdirSync(outDir, { recursive: true });
  console.log(`=== Wcash Kotlin binding (${variant}) ===`);
  run(
    WALLET_BINDGEN,
    [
      'generate',
      UDL,
      '--language',
      'kotlin',
      '--no-format',
      '--out-dir',
      outDir,
    ],
    RUST_DIR,
  );
}
