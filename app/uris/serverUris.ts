import { ServerUrisType, TranslateType, ChainNameEnum } from '@app/AppState';

const serverUris = (
  _translate: (key: string) => TranslateType | void,
): ServerUrisType[] => {
  return [
    // The local Wcash Regtest indexer is the only reviewed endpoint in this
    // build. Add Wcash Testnet here only after its public endpoint is approved.
    {
      uri: 'http://127.0.0.1:48234',
      region: 'Local Regtest',
      chainName: ChainNameEnum.regtestChainName,
      default: true,
      latency: null,
      obsolete: false,
    },
  ];
};

export default serverUris;
