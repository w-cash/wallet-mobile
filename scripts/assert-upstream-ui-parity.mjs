import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = dirname(dirname(fileURLToPath(import.meta.url)));
const roots = ['app', 'screens', 'ui', 'assets'];
const upstreamCount = 445;
const upstreamDigest =
  '1c607a91bc9256e0a200829b6b816c733f07471087b962e3cbf2dd7c1dff68bd';
const reviewedExceptionDigests = new Map([
  [
    'app/uris/wcashMainnetUri.ts',
    '484f226a6e6335d831fc2daf012b30a3495bce96895805b440660a5506d29c61',
  ],
  [
    'app/LoadingApp/LoadingApp.tsx',
    '28fd7516ad00672cff632d4f8421fac9f0f4c442f069f7e72ce50bfc8d39f816',
  ],
  [
    'app/services/SettingsFileImpl.ts',
    'ebe9e7e94d0b7120c769d396cdca6edc2d6ccc32bcf83283695463428e985a69',
  ],
  [
    'screens/ImportUfvk/ImportUfvk.tsx',
    'aabe594ca93183f1aab6624c3d2eaac47c425374b94e3b27fda02d79d679fc19',
  ],
  [
    'app/RPCModule/RPCModule.ts',
    '870d78a8e120be5367e2aa52b6643cd29ef483b9a89240f7d3768f31d3dcfe58',
  ],
  [
    'app/walletBackend/index.ts',
    '40867e41e5cab44f84bc42fbcd011eddce41333adadef2e128f679290cde990a',
  ],
  [
    'app/walletBackend/utils/walletUtils.ts',
    '929e433799cd2519cec7dc4881a8a118cec6acad98cccd6de5b2a7f44080e0fd',
  ],
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
    '23032eb893cd2c0b19eec80dc0af9ac850505e3900bfca8a1c03751e282f51a6',
  ],
  [
    'app/uris/parseServerURI.ts',
    'bf1262306b48ce64eeb0c9ea8a0ad940e0c4c30af6c11cbd93382692a0a5adb1',
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
