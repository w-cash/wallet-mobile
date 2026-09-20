import { ServerUrisType, TranslateType, ChainNameEnum } from '@app/AppState';
import { WCASH_MAINNET_URI } from './wcashMainnetUri';

const serverUris = (
  _translate: (key: string) => TranslateType | void,
): ServerUrisType[] => {
  return [
    {
      uri: WCASH_MAINNET_URI,
      region: 'Wcash Mainnet',
      chainName: ChainNameEnum.mainChainName,
      default: true,
      latency: null,
      obsolete: false,
    },
  ];
};

export default serverUris;
