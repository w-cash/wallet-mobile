import { ChainNameEnum } from '@app/AppState';
import {
  LEGACY_WCASH_MAINNET_URI,
  migrateOfficialWcashMainnetUri,
  WCASH_MAINNET_URI,
} from '@app/uris/wcashMainnetUri';

test('migrates only the former official Wcash Mainnet endpoint', () => {
  expect(
    migrateOfficialWcashMainnetUri(
      LEGACY_WCASH_MAINNET_URI,
      ChainNameEnum.mainChainName,
    ),
  ).toBe(WCASH_MAINNET_URI);
  expect(
    migrateOfficialWcashMainnetUri(
      `${LEGACY_WCASH_MAINNET_URI}/`,
      ChainNameEnum.mainChainName,
    ),
  ).toBe(WCASH_MAINNET_URI);
});

test('preserves custom, testnet, and unrelated endpoints', () => {
  const custom = 'https://wallet.example:443';
  expect(
    migrateOfficialWcashMainnetUri(custom, ChainNameEnum.mainChainName),
  ).toBe(custom);
  expect(
    migrateOfficialWcashMainnetUri(
      LEGACY_WCASH_MAINNET_URI,
      ChainNameEnum.testChainName,
    ),
  ).toBe(LEGACY_WCASH_MAINNET_URI);
  expect(
    migrateOfficialWcashMainnetUri(custom, ChainNameEnum.mainChainName),
  ).toBe(custom);
});

test('migration is idempotent', () => {
  expect(
    migrateOfficialWcashMainnetUri(
      WCASH_MAINNET_URI,
      ChainNameEnum.mainChainName,
    ),
  ).toBe(WCASH_MAINNET_URI);
});
