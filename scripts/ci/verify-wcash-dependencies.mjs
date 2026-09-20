#!/usr/bin/env node

import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const manifest = readFileSync(join(repoRoot, 'rust/Cargo.toml'), 'utf8');
const lock = readFileSync(join(repoRoot, 'rust/Cargo.lock'), 'utf8');
const fullSha = /^[0-9a-f]{40}$/;
const obsoleteRuntime = '5b4e29980eb45e84ddab9024f530c923986d7e1e';
const approvedPins = new Map([
  [
    'https://github.com/w-cash/wallet-core.git',
    new Set(['5bfd56f3ca4f332f9520908821a0b7e166b2372f']),
  ],
  [
    'https://github.com/w-cash/wolf.git',
    new Set(['45393319d24c3c3181f98d503c97a315661b59af']),
  ],
  [
    'https://github.com/zingolabs/zingo-regchest',
    new Set(['4ae67cc9f347077504245dc0082d192db95ac467']),
  ],
  [
    'https://github.com/zingolabs/lightwallet-protocol-rust',
    new Set(['9bdfdc77eb283f2a3d26c27100cc2ac90148cd93']),
  ],
  [
    'https://github.com/zingolabs/infrastructure.git',
    new Set(['89cf104a2416144d1bbbae30aca02bec062d1492']),
  ],
]);

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

if (/path\s*=\s*['"]\//.test(manifest)) {
  fail('rust/Cargo.toml contains an absolute dependency path');
}

if (manifest.includes(obsoleteRuntime) || lock.includes(obsoleteRuntime)) {
  fail(`the dependency graph contains obsolete runtime ${obsoleteRuntime}`);
}

const declarations = [...manifest.matchAll(/^([\w-]+)\s*=\s*\{([\s\S]*?)\}/gm)]
  .map(match => ({ crate: match[1], fields: match[2] }))
  .filter(({ fields }) => /\bgit\s*=/.test(fields));
const gitCount = (manifest.match(/\bgit\s*=/g) ?? []).length;

if (declarations.length !== gitCount || declarations.length === 0) {
  fail('each direct Git dependency must use an inline dependency table');
}

const pinned = declarations.map(({ crate, fields }) => {
  const url = fields.match(/git\s*=\s*"(https:\/\/github\.com\/[\w.-]+\/[\w.-]+(?:\.git)?)"/)?.[1];
  const rev = fields.match(/rev\s*=\s*"([^"]+)"/)?.[1];

  if (!url) fail(`${crate} must use an approved HTTPS Git repository`);
  if (!rev || !fullSha.test(rev)) fail(`${crate} must use a full 40-character Git revision`);
  if (/\b(branch|tag)\s*=/.test(fields)) fail(`${crate} uses a mutable Git selector`);
  if (!approvedPins.get(url)?.has(rev)) fail(`${crate} uses an unreviewed repository revision`);

  return { crate, url, rev };
});

const lockSources = [
  ...lock.matchAll(
    /^source = "git\+(https:\/\/github\.com\/[\w.-]+\/[\w.-]+(?:\.git)?)\?([^"#]+)#([0-9a-f]{40})"$/gm,
  ),
].map(match => ({ url: match[1], query: match[2], resolved: match[3] }));

if (lockSources.length === 0) fail('rust/Cargo.lock contains no Git sources');

for (const source of lockSources) {
  const params = new URLSearchParams(source.query);
  const rev = params.get('rev');
  if ([...params.keys()].some(key => key !== 'rev')) {
    fail(`${source.url} has a mutable or unsupported lockfile selector`);
  }
  if (!rev || !fullSha.test(rev) || rev !== source.resolved) {
    fail(`${source.url} lockfile source does not resolve its exact revision`);
  }
  if (!approvedPins.get(source.url)?.has(source.resolved)) {
    fail(`${source.url} has an unreviewed lockfile revision`);
  }
}

for (const dependency of pinned) {
  const found = lockSources.some(
    source => source.url === dependency.url && source.resolved === dependency.rev,
  );
  if (!found) fail(`${dependency.crate} revision is absent from rust/Cargo.lock`);
}

const walletPackages = [...lock.matchAll(/\[\[package\]\]\nname = "wcash-wallet"\n/g)];
if (walletPackages.length !== 1) fail('rust/Cargo.lock must contain one wcash-wallet package');

const lockedPins = new Map(
  lockSources.map(source => [
    `${source.url}#${source.resolved}`,
    { url: source.url, rev: source.resolved },
  ]),
);

if (process.argv.includes('--remote')) {
  const checkout = mkdtempSync(join(tmpdir(), 'wcash-mobile-revs-'));
  try {
    const init = spawnSync('git', ['init', '--bare', checkout], { stdio: 'inherit' });
    if (init.status !== 0) fail('could not initialize the revision check repository');

    for (const pin of lockedPins.values()) {
      const fetch = spawnSync(
        'git',
        ['-C', checkout, 'fetch', '--quiet', '--no-tags', '--depth=1', pin.url, pin.rev],
        { stdio: 'inherit' },
      );
      if (fetch.status !== 0) fail(`${pin.url} does not publish revision ${pin.rev}`);
    }
  } finally {
    rmSync(checkout, { recursive: true, force: true });
  }
}

console.log(
  `verified ${pinned.length} direct Git declarations and ${lockedPins.size} locked revisions`,
);
