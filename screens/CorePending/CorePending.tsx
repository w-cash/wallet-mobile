import React from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { WcashProduct } from '@app/product/WcashProduct';
import { useTheme } from '@app/theme';

const CorePending = () => {
  const { colors } = useTheme();

  return (
    <View style={[styles.page, { backgroundColor: colors.bgCanvas }]}>
      <View
        accessibilityLabel={WcashProduct.displayName}
        style={[
          styles.mark,
          {
            backgroundColor: colors.bgSurface,
            borderColor: colors.borderAccent,
          },
        ]}
      >
        <Text style={[styles.markText, { color: colors.fgAccent }]}>W</Text>
      </View>
      <Text style={[styles.title, { color: colors.fgDefault }]}>
        {WcashProduct.displayName}
      </Text>
      <View
        style={[
          styles.network,
          {
            backgroundColor: colors.bgAccentDisabled,
            borderColor: colors.borderAccent,
          },
        ]}
      >
        <Text style={[styles.networkText, { color: colors.fgDefault }]}>
          {WcashProduct.networkName} · {WcashProduct.ticker}
        </Text>
      </View>
      <Text style={[styles.status, { color: colors.fgMuted }]}>
        The reviewed Wcash wallet core is pending.
      </Text>
      <Text style={[styles.detail, { color: colors.fgMuted }]}>
        This build holds wallet access until the Testnet core is pinned.
      </Text>
    </View>
  );
};

const styles = StyleSheet.create({
  page: {
    alignItems: 'center',
    flex: 1,
    justifyContent: 'center',
    paddingHorizontal: 32,
  },
  mark: {
    alignItems: 'center',
    borderRadius: 28,
    borderWidth: 1,
    height: 96,
    justifyContent: 'center',
    width: 96,
  },
  markText: {
    fontSize: 54,
    fontWeight: '700',
  },
  title: {
    fontSize: 30,
    fontWeight: '700',
    marginTop: 28,
    textAlign: 'center',
  },
  network: {
    borderRadius: 999,
    borderWidth: 1,
    marginTop: 18,
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  networkText: {
    fontSize: 13,
    fontWeight: '600',
    letterSpacing: 0.5,
  },
  status: {
    fontSize: 17,
    fontWeight: '600',
    marginTop: 34,
    textAlign: 'center',
  },
  detail: {
    fontSize: 15,
    lineHeight: 22,
    marginTop: 10,
    maxWidth: 420,
    textAlign: 'center',
  },
});

export default CorePending;
