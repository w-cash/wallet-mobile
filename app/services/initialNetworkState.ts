import NetInfo, {
  NetInfoState,
  NetInfoStateType,
} from '@react-native-community/netinfo';

const INITIAL_NETWORK_TIMEOUT_MS = 5000;

const unknownNetworkState = (): NetInfoState => ({
  type: NetInfoStateType.unknown,
  isConnected: null,
  isInternetReachable: null,
  details: null,
});

export async function fetchInitialNetworkState(
  fetchState: () => Promise<NetInfoState> = () => NetInfo.fetch(),
  timeoutMs: number = INITIAL_NETWORK_TIMEOUT_MS,
): Promise<NetInfoState> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const timeoutState = new Promise<NetInfoState>(resolve => {
    timeout = setTimeout(() => resolve(unknownNetworkState()), timeoutMs);
  });

  try {
    return await Promise.race([fetchState(), timeoutState]);
  } catch {
    return unknownNetworkState();
  } finally {
    if (timeout !== undefined) {
      clearTimeout(timeout);
    }
  }
}

export const canAttemptNetwork = (state: NetInfoState): boolean =>
  state.isConnected !== false;
