import {
  canAttemptNetwork,
  fetchInitialNetworkState,
} from '@app/services/initialNetworkState';
import {
  NetInfoState,
  NetInfoStateType,
} from '@react-native-community/netinfo';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const connected: NetInfoState = {
  type: NetInfoStateType.wifi,
  isConnected: true,
  isInternetReachable: true,
  details: {
    isConnectionExpensive: false,
    ssid: null,
    bssid: null,
    strength: null,
    ipAddress: null,
    subnet: null,
    frequency: null,
    linkSpeed: null,
    rxLinkSpeed: null,
    txLinkSpeed: null,
  },
};

describe('initial network state', () => {
  afterEach(() => jest.useRealTimers());

  it('returns the native state when reachability settles', async () => {
    await expect(
      fetchInitialNetworkState(async () => connected, 10),
    ).resolves.toBe(connected);
  });

  it('falls back to unknown when native reachability stays pending', async () => {
    jest.useFakeTimers();
    const result = fetchInitialNetworkState(
      () => new Promise<NetInfoState>(() => undefined),
      5000,
    );

    await jest.advanceTimersByTimeAsync(5000);
    await expect(result).resolves.toMatchObject({
      type: NetInfoStateType.unknown,
      isConnected: null,
      isInternetReachable: null,
    });
  });

  it('falls back to unknown when native reachability rejects', async () => {
    await expect(
      fetchInitialNetworkState(async () => {
        throw new Error('native reachability unavailable');
      }, 10),
    ).resolves.toMatchObject({
      type: NetInfoStateType.unknown,
      isConnected: null,
    });
  });

  it('allows the wallet server probe for unknown and blocks explicit offline', () => {
    const unknown = {
      type: NetInfoStateType.unknown,
      isConnected: null,
      isInternetReachable: null,
      details: null,
    } as NetInfoState;
    const offline = {
      type: NetInfoStateType.none,
      isConnected: false,
      isInternetReachable: false,
      details: null,
    } as NetInfoState;

    expect(canAttemptNetwork(unknown)).toBe(true);
    expect(canAttemptNetwork(offline)).toBe(false);
  });

  test('Tests that basic first-run creation accepts unknown reachability.', () => {
    const loadingApp = readFileSync(
      join(process.cwd(), 'app/LoadingApp/LoadingApp.tsx'),
      'utf8',
    );

    expect(loadingApp).toContain(
      '!canAttemptNetwork(netInfoState) ||\n            this.state.selectServer === SelectServerEnum.offline',
    );
  });
});
