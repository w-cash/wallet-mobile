import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = dirname(dirname(fileURLToPath(import.meta.url)));
const roots = ['app', 'screens', 'ui', 'assets'];
const upstreamCount = 450;
const upstreamDigest =
  '84f4906a7158fe701a1f6b04058cdc9ade8ff59a8a7d1c7e76ab4283463600a0';
const brandedDigests = new Map([
  [
    'assets/img/logobig-zingo-beta.png',
    'c878506b87ee8497b7ae3d0153fdd9c2dbda6732e05361722945f36d94be70b5',
  ],
  [
    'assets/img/logobig-zingo.png',
    'c878506b87ee8497b7ae3d0153fdd9c2dbda6732e05361722945f36d94be70b5',
  ],
  [
    'assets/img/logoiosopaque-zingo.png',
    'b37a92a7266762961e3c5582451a69a7bf90a4abd3afcb4875cc0287b4bb8829',
  ],
  [
    'app/AppErrorBoundary.tsx',
    'deaf1cf3a66676b1a0471c91bcc7d830b592415c26965d91a8fcaec3a71a07b5',
  ],
  [
    'app/AppState/const/GlobalConst.ts',
    'fabfefa60fb6c8b32a63d82a77527827420fc59659c5c651e67d4f3bd9c3a3c4',
  ],
  [
    'app/AppState/enums/CurrencyNameEnum.ts',
    '112901a21c21977acca2b0af1d1d9e1c29bfef69e8c7e94203ab3d51dbaec6e1',
  ],
  [
    'app/LoadedApp/LoadedApp.tsx',
    'e58ea3b9ad0a90ce13a5781925c3c1d1048cb5220235d25f22945a0a765b1906',
  ],
  [
    'app/LoadedApp/LoadedAppOptionsPanelHost.tsx',
    '5d2a69d37ae6114db8f16ddd3a24d07633dbe22b8a151b4f0c102df61c790b7f',
  ],
  [
    'app/LoadingApp/LoadingApp.tsx',
    '76d4826c1b9884ff02ab15a49ff9fbb7b9c9577da065ffa6524ceed30f748145',
  ],
  [
    'app/services/SettingsFileImpl.ts',
    '8faa4d61c3def2e921aae2baf5456bee6d8aaf20c5a55096066accf55d61c550',
  ],
  [
    'app/services/sendEmail.ts',
    '9c4545888c07ffc8cc3a92331c401be71a6e21949c3f9471ce236d075df6bf96',
  ],
  [
    'app/translations/en.json',
    'b308568a27bbc4e426e1a3ab6d242be651f227fcb6738a9bac40b2bc63fde92d',
  ],
  [
    'app/translations/es.json',
    '0536717f8018fb56a364efff6df739abb0d48bb90c46b6a86258e99435234d9c',
  ],
  [
    'app/translations/pt.json',
    '92ac7f17e42156cda97e2b2f2346196d0c94d9c25e2eae781c232921191403fa',
  ],
  [
    'app/translations/ru.json',
    'ba7b7118ba3689eb997199aeabf679c965b0cd8d24cea7b2b6b5c1bcb7a33cba',
  ],
  [
    'app/translations/tr.json',
    'ac43caeb7f6268b4ec16ea567026f405a54ccfc8d789dedbaa85c139778251a8',
  ],
  [
    'app/uris/fetchServerList.ts',
    '39e483824ebcbd52d2cb5bc62b3d63fdb6eb6e7ed1a2120e608c222830af0325',
  ],
  [
    'app/uris/resolveZnsName.ts',
    'fc2ec3d50291ee275ad01f0cc3ce816190f83db3279aec41cb9e3916cdda687a',
  ],
  [
    'app/uris/serverUris.ts',
    'b7c0f7b203d3790586868a8e1dc7cd8884d91eb75556461b0e61e2bfee6455cd',
  ],
  [
    'app/utils/Utils.ts',
    'e0f17862e7293cd6a70416d791c01eac7dbe1b2b160c0b129e42262568545a32',
  ],
  [
    'app/utils/ZingoAppData.ts',
    '0d1a3cdc74d5f801147ab2ce0e36e97daeca25c0c384904070a4f15122ffb94d',
  ],
  [
    'app/walletBackend/modules/DataService.ts',
    'e90cdfdf87b189a5d772189bc1438d061a78dbc63745c9fd81efae07ceace010',
  ],
  [
    'ui/widgets/Header/components/PriceRow.tsx',
    'cf74fa8adec907f2fd19d6d973ce7ad93e142aa62fef45b72997a370f9f9bb0c',
  ],
  [
    'ui/widgets/chainDisplayName.ts',
    'bf9b51a82de498126535d93b68c5eb79bf21d11442b0cb1a1034f9fce2d3d567',
  ],
]);

const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');

const filesBelow = async (dir) => {
  const entries = await readdir(dir, { withFileTypes: true });
  const files = await Promise.all(
    entries.map((entry) => {
      const path = join(dir, entry.name);
      return entry.isDirectory() ? filesBelow(path) : [path];
    }),
  );
  return files.flat();
};

const paths = (
  await Promise.all(roots.map((root) => filesBelow(join(repo, root))))
)
  .flat()
  .map((path) => ({
    path,
    relativePath: relative(repo, path).split(sep).join('/'),
  }))
  .sort((left, right) =>
    left.relativePath < right.relativePath
      ? -1
      : left.relativePath > right.relativePath
        ? 1
        : 0,
  );

const upstreamPaths = paths.filter(
  ({ relativePath }) => !brandedDigests.has(relativePath),
);
if (upstreamPaths.length !== upstreamCount) {
  throw new Error(
    `Expected ${upstreamCount} upstream UI files, found ${upstreamPaths.length}`,
  );
}

const combined = createHash('sha256');
for (const { path, relativePath } of upstreamPaths) {
  combined.update(relativePath);
  combined.update('\0');
  combined.update(await readFile(path));
  combined.update('\0');
}
const actualUpstreamDigest = combined.digest('hex');
if (actualUpstreamDigest !== upstreamDigest) {
  throw new Error(`Upstream UI digest changed: ${actualUpstreamDigest}`);
}

for (const [relativePath, expectedDigest] of brandedDigests) {
  const actualDigest = digest(await readFile(join(repo, relativePath)));
  if (actualDigest !== expectedDigest) {
    throw new Error(`Branding file changed: ${relativePath} ${actualDigest}`);
  }
}

console.log(
  `Verified ${upstreamCount} byte-identical upstream UI files and ${brandedDigests.size} branding files.`,
);
