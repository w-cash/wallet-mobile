import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = dirname(dirname(fileURLToPath(import.meta.url)));
const roots = ['app', 'screens', 'ui', 'assets'];
const upstreamCount = 452;
const upstreamDigest =
  'c14027c3a01b23eff5b6675bb03db3835e306f1d90333596e9e98a69e9f7220e';
const reviewedExceptionDigests = new Map([
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
    '1009ddd9ca7097c4560a1e2bfb77d18b4c36295f038d9e184fb25758bf67764b',
  ],
  [
    'app/AppState/const/GlobalConst.ts',
    '6588b47872f53d8b3a0d63b968eb02c2fdf12b6e207a38dabf228420fe3e39bd',
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
    'app/services/sendEmail.ts',
    '9c4545888c07ffc8cc3a92331c401be71a6e21949c3f9471ce236d075df6bf96',
  ],
  [
    'app/translations/en.json',
    '0beb347c7e89e366acd35f339983d425e352fe636352b77a1bff9fedc37c4e40',
  ],
  [
    'app/translations/es.json',
    '77781fe852d6fd5115cce56b18187ec9a436b01bfe3d6eb2af640ef51921aed2',
  ],
  [
    'app/translations/pt.json',
    'cd4ecaef1661e8ddb241ad2ab057d9378f44cf581d4d6634cc5b1dbc5ea3c04a',
  ],
  [
    'app/translations/ru.json',
    'b99f3ca8881eec4ce168a9263ef570ddf6ecc846b4d74955df47197ba154821b',
  ],
  [
    'app/translations/tr.json',
    '67aee8e1948161030fef83d06b85b8f73064a16040a1276e753b2c3d7855fbaf',
  ],
  [
    'app/uris/fetchServerList.ts',
    '39e483824ebcbd52d2cb5bc62b3d63fdb6eb6e7ed1a2120e608c222830af0325',
  ],
  [
    'app/uris/resolveZnsName.ts',
    'e3f5adc67bac803a5c09bc2c0d0e1295e743c633230b91b7979b26c52cbbab45',
  ],
  [
    'app/uris/serverUris.ts',
    'b7c0f7b203d3790586868a8e1dc7cd8884d91eb75556461b0e61e2bfee6455cd',
  ],
  [
    'app/utils/Utils.ts',
    '0fb579c13a7016f08a3f6eb7087a832e0f016a1d00a6cabac55b5ad5a3223b3c',
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

const digest = bytes => createHash('sha256').update(bytes).digest('hex');

const filesBelow = async dir => {
  const entries = await readdir(dir, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(entry => {
      const path = join(dir, entry.name);
      return entry.isDirectory() ? filesBelow(path) : [path];
    }),
  );
  return files.flat();
};

const paths = (
  await Promise.all(roots.map(root => filesBelow(join(repo, root))))
)
  .flat()
  .map(path => ({
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
  ({ relativePath }) => !reviewedExceptionDigests.has(relativePath),
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

for (const [relativePath, expectedDigest] of reviewedExceptionDigests) {
  const actualDigest = digest(await readFile(join(repo, relativePath)));
  if (actualDigest !== expectedDigest) {
    throw new Error(
      `Reviewed UI exception changed: ${relativePath} ${actualDigest}`,
    );
  }
}

console.log(
  `Verified ${upstreamCount} byte-identical upstream UI files and ${reviewedExceptionDigests.size} reviewed UI exceptions.`,
);
