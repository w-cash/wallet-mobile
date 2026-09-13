#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const workflows = [
  '.github/workflows/wcash-pr-qa.yaml',
  '.github/workflows/android-release.yaml',
  '.github/workflows/mobile-production-signing.yaml',
];
const approved = new Map([
  ['actions/checkout', '3d3c42e5aac5ba805825da76410c181273ba90b1'],
  ['actions/setup-node', '49933ea5288caeca8642d1e84afbd3f7d6820020'],
  ['actions/setup-java', 'de7274f081f381c8f8158605e0321c36c376e2e6'],
  ['actions/upload-artifact', 'ea165f8d65b6e75b540449e92b4886f43607fa02'],
  ['actions/download-artifact', '634f93cb2916e3fdff6788551b99b062d0335ce0'],
  ['android-actions/setup-android', '9fc6c4e9069bf8d3d10b2204b1fb8f6ef7065407'],
  ['gradle/actions/setup-gradle', '9c971963bec38e04b3d30dcc455b5382be2fdbfb'],
  ['dtolnay/rust-toolchain', '8d8cc9a8e0d47b64af669de71110e76860d6afbd'],
  ['Swatinem/rust-cache', '6323deb102c322ba6fcbdcafc7e3dddab59af2b6'],
  ['maxim-lobanov/setup-xcode', 'ed7a3b1fda3918c0306d1b724322adc0b8cc0a90'],
  ['ruby/setup-ruby', '95ef2b042f9d7a56d8268cba8559e2842e2ad01b'],
]);

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

let usesCount = 0;
for (const workflow of workflows) {
  const yaml = readFileSync(join(repoRoot, workflow), 'utf8');
  for (const match of yaml.matchAll(/^\s*uses:\s*([^\s]+)\s*$/gm)) {
    const target = match[1];
    if (target.startsWith('./')) continue;
    usesCount += 1;
    const separator = target.lastIndexOf('@');
    const action = target.slice(0, separator);
    const revision = target.slice(separator + 1);
    if (separator < 1 || !/^[0-9a-f]{40}$/.test(revision)) {
      fail(`${workflow} contains an unpinned action: ${target}`);
    }
    if (approved.get(action) !== revision) {
      fail(`${workflow} contains an unreviewed action revision: ${target}`);
    }
  }
}

if (usesCount === 0) fail('the Wcash workflows contain no external actions');
console.log(`verified ${usesCount} immutable workflow action references`);
