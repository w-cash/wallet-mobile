jest.mock('react-native', () => ({ Platform: { OS: 'ios' } }));

jest.mock('react-native-keychain', () => ({
  ACCESSIBLE: { WHEN_UNLOCKED_THIS_DEVICE_ONLY: 'whenUnlocked' },
  ACCESS_CONTROL: {
    BIOMETRY_ANY_OR_DEVICE_PASSCODE: 'biometryOrPasscode',
  },
  SECURITY_LEVEL: { SECURE_HARDWARE: 'secureHardware' },
  STORAGE_TYPE: {
    AES_GCM: 'aesGcm',
    AES_GCM_NO_AUTH: 'aesGcmNoAuth',
  },
  getGenericPassword: jest.fn(),
  setGenericPassword: jest.fn(),
  resetGenericPassword: jest.fn(),
}));

import { GlobalConst } from '@app/AppState';
import { hasRecoveryWalletInfo } from '@app/services/recoveryWalletInfo';

const keychain = require('react-native-keychain');

describe('recovery wallet presence', () => {
  beforeEach(() => keychain.getGenericPassword.mockReset());

  it('detects the recovery entry by reading the supported credential API', async () => {
    keychain.getGenericPassword.mockResolvedValue({
      username: GlobalConst.keyKeyChain,
      password: '{}',
      service: GlobalConst.serviceKeyChain,
      storage: 'keychain',
    });

    await expect(hasRecoveryWalletInfo()).resolves.toBe(true);
  });

  it('does not accept a credential from another key or service', async () => {
    keychain.getGenericPassword.mockResolvedValue({
      username: 'other',
      password: '{}',
      service: GlobalConst.serviceKeyChain,
      storage: 'keychain',
    });

    await expect(hasRecoveryWalletInfo()).resolves.toBe(false);
  });

  it('treats an unavailable keychain entry as absent', async () => {
    keychain.getGenericPassword.mockRejectedValue(new Error('unavailable'));

    await expect(hasRecoveryWalletInfo()).resolves.toBe(false);
  });
});
