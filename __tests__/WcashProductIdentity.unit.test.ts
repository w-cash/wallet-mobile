import { WcashProduct } from '@app/product/WcashProduct';

test('Tests that the Wcash shell holds wallet access when the core pin is pending.', () => {
  expect(WcashProduct).toMatchObject({
    displayName: 'Wcash Warden Testnet',
    networkName: 'Wcash Testnet',
    ticker: 'TWC',
    storageNamespace: 'com.wcashwallet.warden.testnet',
    walletCoreReady: false,
    networkSelectionEnabled: false,
  });
});
