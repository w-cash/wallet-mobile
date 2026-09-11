/**
 * @format
 */

import React from 'react';
import 'react-native';

import { render } from '@testing-library/react-native';
import App from '@app/App';

jest.mock('@app/LoadingApp', () => ({ LoadingApp: jest.fn(() => null) }));
jest.mock('@app/LoadedApp', () => ({ LoadedApp: jest.fn(() => null) }));

describe('Wcash product gate', () => {
  test('Tests that the pending-core screen renders when the Wcash app starts.', () => {
    const screen = render(<App />);

    expect(screen.getByText('Wcash Warden Testnet')).toBeTruthy();
    expect(screen.getByText('Wcash Testnet · TWC')).toBeTruthy();
    expect(
      screen.getByText(
        'This build holds wallet access until the Testnet core is pinned.',
      ),
    ).toBeTruthy();
    const loadingApp = jest.requireMock('@app/LoadingApp').LoadingApp;
    const loadedApp = jest.requireMock('@app/LoadedApp').LoadedApp;

    expect(loadingApp).not.toHaveBeenCalled();
    expect(loadedApp).not.toHaveBeenCalled();
  });
});
