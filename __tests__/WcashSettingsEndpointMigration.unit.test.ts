jest.mock('react-native-fs', () => ({
  DocumentDirectoryPath: '/wallet',
  exists: jest.fn(),
  readFile: jest.fn(),
  writeFile: jest.fn(),
}));

jest.mock('@app/uris', () => ({
  serverUris: () => [
    {
      uri: 'https://mainnet.zecwec.com:443',
      chainName: 'main',
      default: true,
      obsolete: false,
    },
  ],
}));

import * as RNFS from 'react-native-fs';

import {
  ChainNameEnum,
  SelectServerEnum,
  SettingsFileClass,
} from '@app/AppState';
import SettingsFileImpl from '@app/services/SettingsFileImpl';
import {
  LEGACY_WCASH_MAINNET_URI,
  WCASH_MAINNET_URI,
} from '@app/uris/wcashMainnetUri';

const exists = RNFS.exists as jest.MockedFunction<typeof RNFS.exists>;
const readFile = RNFS.readFile as jest.MockedFunction<typeof RNFS.readFile>;
const writeFile = RNFS.writeFile as jest.MockedFunction<typeof RNFS.writeFile>;

const storedSettings = (uri: string, chainName: ChainNameEnum) =>
  ({
    firstInstall: false,
    version: null,
    selectServer: SelectServerEnum.custom,
    server: { uri, chainName },
  }) as SettingsFileClass;

beforeEach(() => {
  jest.clearAllMocks();
  exists.mockResolvedValue(true);
  writeFile.mockResolvedValue(undefined);
});

test('persists migration of the former official Mainnet endpoint', async () => {
  readFile.mockResolvedValue(
    JSON.stringify(
      storedSettings(LEGACY_WCASH_MAINNET_URI, ChainNameEnum.mainChainName),
    ),
  );

  const settings = await SettingsFileImpl.readSettings();

  expect(settings.server).toEqual({
    uri: WCASH_MAINNET_URI,
    chainName: ChainNameEnum.mainChainName,
  });
  expect(writeFile).toHaveBeenCalledTimes(1);
  expect(writeFile).toHaveBeenCalledWith(
    '/wallet/settings.json',
    expect.stringContaining(WCASH_MAINNET_URI),
    'utf8',
  );
});

test.each([
  ['custom Mainnet', 'https://wallet.example:443', ChainNameEnum.mainChainName],
  [
    'legacy URL on Testnet',
    LEGACY_WCASH_MAINNET_URI,
    ChainNameEnum.testChainName,
  ],
])(
  'preserves %s settings without rewriting them',
  async (_label, uri, chainName) => {
    readFile.mockResolvedValue(JSON.stringify(storedSettings(uri, chainName)));

    const settings = await SettingsFileImpl.readSettings();

    expect(settings.server).toEqual({ uri, chainName });
    expect(writeFile).not.toHaveBeenCalled();
  },
);
