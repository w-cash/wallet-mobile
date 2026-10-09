import { ChainNameEnum } from '@app/AppState';

export const WCASH_MAINNET_URI = 'https://mainnet.zecwec.com:443';
export const LEGACY_WCASH_MAINNET_URI = 'http://mainnet.zecwec.com:48234';

const withoutTrailingSlash = (uri: string): string => uri.replace(/\/+$/, '');

/** Migrates the former official Mainnet endpoint and preserves every other endpoint. */
export const migrateOfficialWcashMainnetUri = (
  uri: string,
  chainName: ChainNameEnum,
): string => {
  if (
    chainName === ChainNameEnum.mainChainName &&
    withoutTrailingSlash(uri) === LEGACY_WCASH_MAINNET_URI
  ) {
    return WCASH_MAINNET_URI;
  }
  return uri;
};
