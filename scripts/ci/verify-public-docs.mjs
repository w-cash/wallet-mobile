#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const paths = [
  'README.md',
  'SECURITY.md',
  'SUPPORT.md',
  'docs/release_quickstart.md',
];
const docs = new Map(
  paths.map(path => [path, readFileSync(join(root, path), 'utf8')]),
);
const prose = new Map(
  [...docs].map(([path, text]) => [path, text.replace(/\s+/g, ' ')]),
);

function fail(message) {
  console.error(`error: ${message}`);
  process.exitCode = 1;
}

const inheritedRoutes = [
  'apps.apple.com/app/zingo',
  'play.google.com/store/apps/details?id=org.ZingoLabs.Zingo',
  'zingodisclosure@proton.me',
  'support@zingolabs.org',
];

for (const [path, text] of docs) {
  for (const route of inheritedRoutes) {
    if (text.includes(route)) fail(`${path} contains inherited route ${route}`);
  }
}

const required = [
  [
    'README.md',
    'https://github.com/w-cash/wallet-mobile/releases/tag/wcash-2.0.23-317',
  ],
  ['README.md', 'f136a09d7b4959ee800dcef6d4cb9a0b4b39b1da'],
  ['README.md', 'Android debug certificate'],
  ['README.md', 'unsigned iOS'],
  ['README.md', 'does not authenticate the publisher'],
  [
    'SECURITY.md',
    'https://github.com/w-cash/wallet-mobile/security/advisories/new',
  ],
  ['SECURITY.md', 'must enable and verify Private Vulnerability Reporting'],
  ['SUPPORT.md', 'no supported consumer release'],
  ['docs/release_quickstart.md', 'Wcash-Wallet-mainnet-<version>'],
];

for (const [path, phrase] of required) {
  if (!prose.get(path).includes(phrase)) fail(`${path} must contain ${phrase}`);
}

if (!process.exitCode) console.log('public documentation checks passed');
