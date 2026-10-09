import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = dirname(dirname(fileURLToPath(import.meta.url)));
const roots = ['app', 'screens', 'ui', 'assets'];
const upstreamCount = 441;
const upstreamDigest =
  '9abe32ee8898b50edf3aa96a7a46ff2adb0d2e18cf3f7d8d54b92a2627c1d379';
const reviewedExceptionDigests = new Map([
  [
    'app/uris/wcashMainnetUri.ts',
    '484f226a6e6335d831fc2daf012b30a3495bce96895805b440660a5506d29c61',
  ],
  [
    'app/LoadingApp/LoadingApp.tsx',
    '3ef1a834fe03daea79393311465969d0b6b89455ae4e31e55fb92efefce9664c',
  ],
  [
    'app/services/SettingsFileImpl.ts',
    '3bb789bcfec9a02d5922625ee11b65d3a8569098e0476b5be11ac1bd2d171b28',
  ],
  [
    'app/services/initialNetworkState.ts',
    '1485510e85d282a196b9e9cfd7937e6b3cc0c88c5a8200a43c4736588faa06b9',
  ],
  [
    'app/services/recoveryWalletInfo.ts',
    'd81c617e5a62e6570b2924d999f62d17daa9a07faee1d60466832c07330ac98c',
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
    '6991dabed63124e8da9a5027d23abc6e4e89c687e34e47f8849d1187a6364224',
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
    '8ff75830ad71eb395c9e1df51f4a32ab677caee2bae2457fd010ffea65af1df7',
  ],
  [
    'app/translations/es.json',
    '3c70a5c639b55b324fa0980127987133cb8a7b99013fc2ccc332bccb4f7f74b6',
  ],
  [
    'app/translations/pt.json',
    '4a6d5fc1fb3329052aa309716995f43f8a4a40bc4b37b4f95f5beef98de23046',
  ],
  [
    'app/translations/ru.json',
    '5455e09bfc39eaffe84dcd279d3629e97eb31612d966bf1818a9bb837fb66e8a',
  ],
  [
    'app/translations/tr.json',
    'f3dffa7da0e03e812d2f5b5a558ca62ca56ead03ff1d465f46cf214f1ed3907b',
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
  [
    'ui/widgets/ChainSelect.tsx',
    'c864e579519888fe67e0c9f0c81491030f9b2b7f9ef0a6f2bb5376fc37d3dff3',
  ],
  [
    'ui/widgets/ZecAmount.tsx',
    '36dae1b7ba02a60a474f39eceaced84bff36764255fad8e07f4fb0bfe0a8cda7',
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
