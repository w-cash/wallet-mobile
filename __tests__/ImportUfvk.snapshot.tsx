/**
 * @format
 */

import 'react-native';
import React from 'react';

import { render, waitFor } from '@testing-library/react-native';
import ImportUfvk from '@screens/ImportUfvk';
import RPCModule from '@app/RPCModule';
import {
  ContextAppLoadedProvider,
  defaultAppContextLoaded,
} from '@app/context';
import { mockTranslate } from '../__mocks__/dataMocks/mockTranslate';
import { mockInfo } from '../__mocks__/dataMocks/mockInfo';
import { mockTotalBalance } from '../__mocks__/dataMocks/mockTotalBalance';

// test suite
describe('Component ImportUfvk - test', () => {
  //snapshot test
  test('ImportUfvk - snapshot', async () => {
    const state = { ...defaultAppContextLoaded };
    state.translate = mockTranslate;
    state.info = mockInfo;
    state.totalBalance = mockTotalBalance;
    const onCancel = jest.fn();
    const onOK = jest.fn();
    const importUfvk = render(
      <ContextAppLoadedProvider value={state}>
        <ImportUfvk onClickCancel={onCancel} onClickOK={onOK} />
      </ContextAppLoadedProvider>,
    );
    await waitFor(() =>
      expect(JSON.stringify(importUfvk.toJSON())).toContain('(1, --)'),
    );
    expect(importUfvk.toJSON()).toMatchSnapshot();
  });

  test('Tests that the import form uses the native activation height.', async () => {
    const nativeActivation = jest
      .spyOn(RPCModule, 'getWalletActivationHeight')
      .mockResolvedValueOnce('73');
    const state = { ...defaultAppContextLoaded };
    state.translate = mockTranslate;
    state.info = mockInfo;
    state.totalBalance = mockTotalBalance;
    const importUfvk = render(
      <ContextAppLoadedProvider value={state}>
        <ImportUfvk onClickCancel={jest.fn()} onClickOK={jest.fn()} />
      </ContextAppLoadedProvider>,
    );

    await waitFor(() =>
      expect(JSON.stringify(importUfvk.toJSON())).toContain('(73, --)'),
    );
    expect(nativeActivation).toHaveBeenLastCalledWith(state.server.chainName);
  });
});
