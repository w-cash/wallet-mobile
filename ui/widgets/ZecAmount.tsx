/* eslint-disable react-native/no-inline-styles */
import React, { useState, useEffect } from 'react';
import {
  Text,
  View,
  Platform,
  TextStyle,
  TouchableOpacity,
} from 'react-native';
import { useTheme } from '@app/theme';
import { getNumberFormatSettings } from 'react-native-localize';

import Utils from '@app/utils';
import { CurrencyNameEnum, GlobalConst } from '@app/AppState';

type ZecAmountProps = {
  color?: string;
  size?: number;
  amtZec?: number;
  style?: TextStyle;
  currencyName?: CurrencyNameEnum;
  privacy?: boolean;
  smallPrefix?: boolean;
  testID?: string;
};

const ZecAmount: React.FunctionComponent<ZecAmountProps> = ({
  color,
  style,
  size,
  currencyName,
  amtZec,
  privacy,
  smallPrefix,
  testID,
}) => {
  const [privacyHigh, setPrivacyHigh] = useState<boolean>(privacy || false);
  const splits = Utils.splitZecAmountIntoBigSmall(amtZec);
  const { colors } = useTheme();
  const { decimalSeparator } = getNumberFormatSettings();

  useEffect(() => {
    setPrivacyHigh(privacy || false);
  }, [privacy]);

  useEffect(() => {
    if (privacyHigh && !privacy) {
      setPrivacyHigh(false);
    }
  }, [privacyHigh, privacy]);

  if (!size) {
    size = 24;
  }

  if (!color) {
    color = colors.fgDefault;
  }

  if (!smallPrefix) {
    smallPrefix = false;
  }

  const onPress = () => {
    setPrivacyHigh(false);
    setTimeout(() => setPrivacyHigh(true), 5 * 1000);
  };

  return (
    <View style={{ ...style, flexDirection: 'row', marginHorizontal: 5 }}>
      <TouchableOpacity disabled={!privacyHigh} onPress={onPress}>
        <View
          testID={testID}
          style={{
            flexDirection: 'row',
            alignItems: 'flex-end',
            margin: 0,
            padding: 0,
          }}
        >
          <Text
            testID={testID ? `${testID}.currency-name` : undefined}
            style={{
              fontSize: size * (smallPrefix ? 0.7 : 1),
              fontWeight: currencyName === CurrencyNameEnum.ZEC ? '700' : '400',
              color,
              margin: 0,
              padding: 0,
            }}
          >
            {currencyName ? currencyName : '---'}
          </Text>
          {privacyHigh ? (
            <Text
              style={{
                fontSize: size,
                fontWeight: '700',
                color,
                margin: 0,
                padding: 0,
              }}
            >
              {' -' + decimalSeparator + '----'}
            </Text>
          ) : (
            <Text
              testID={`${testID}.big-part`}
              style={{
                fontSize: size,
                fontWeight: '700',
                color,
                margin: 0,
                padding: 0,
              }}
            >
              {' ' + splits.bigPart}
            </Text>
          )}
          {splits.smallPart !== '0000' && !privacyHigh && (
            <Text
              testID={`${testID}.small-part`}
              style={{
                fontSize: size * 0.7,
                color,
                margin: 0,
                padding: 0,
                marginBottom:
                  Platform.OS === GlobalConst.platformOSandroid
                    ? size / 10
                    : size / 15,
              }}
            >
              {splits.smallPart}
            </Text>
          )}
        </View>
      </TouchableOpacity>
    </View>
  );
};

export default ZecAmount;
