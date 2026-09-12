/**
 * ZNS is a Zcash service. Wcash has no reviewed name-service registry, so the
 * unchanged Send field must fail closed without contacting that service.
 */
import { ZNS } from 'zcashname-sdk';
import { ChainNameEnum } from '@app/AppState';
import { isZnsAlias, resolveZnsName } from '@app/uris/resolveZnsName';

describe('Wcash name-service policy', () => {
  afterEach(() => {
    jest.restoreAllMocks();
  });

  it.each([
    'alice.zcash',
    'alice.zec',
    'ALICE.ZCASH',
    '  bob123.zcash  ',
    'alice',
    '',
  ])(
    'does not classify %s as an active Wcash alias',
    text => {
      expect(isZnsAlias(text)).toBe(false);
    },
  );

  it.each([
    ChainNameEnum.mainChainName,
    ChainNameEnum.testChainName,
    ChainNameEnum.regtestChainName,
  ])('fails closed on %s without querying the Zcash indexer', async chain => {
    const resolveName = jest.spyOn(ZNS.prototype, 'resolveName');

    await expect(resolveZnsName('alice.zcash', chain)).resolves.toEqual({
      ok: false,
      reason: 'unsupported-chain',
    });
    expect(resolveName).not.toHaveBeenCalled();
  });
});
