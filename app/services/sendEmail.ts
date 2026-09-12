import { Linking } from 'react-native';
import DeviceInfo from 'react-native-device-info';
import { GlobalConst, TranslateType } from '@app/AppState';
import { sanitizePaths } from '@app/utils/sanitizePaths';
import { getZingoName, getZingoVersion } from '@app/utils/ZingoAppData';
import { showConfirm } from './showConfirm';

const SUPPORT_ISSUES_URL = 'https://github.com/w-cash/wallet-mobile/issues';

export const sendEmail = async (
  translate: (key: string) => TranslateType,
  zingolibVersion: string,
  subject?: string,
  body?: string,
) => {
  // Sanitize subject + body so a stack-trace or path baked into an error
  // never leaks the developer's username (Hermes embeds compile-time
  // absolute paths into release stacks) or the user's profile folder.
  const subjectEmail: string = sanitizePaths(
    subject || (translate('subject') as string),
  );
  const bodyEmail: string = sanitizePaths(
    body || (translate('body') as string),
  );
  const appLabel: string = `${getZingoName()} ${getZingoVersion()}`;
  const zingolibVersionEmail: string = zingolibVersion;
  const systemName = DeviceInfo.getSystemName();
  const systemVersion = DeviceInfo.getSystemVersion();
  const manufacturer = await DeviceInfo.getManufacturer();
  const model = DeviceInfo.getModel();

  const url = `${SUPPORT_ISSUES_URL}/new?title=${encodeURIComponent(subjectEmail)}&body=${encodeURIComponent(
    manufacturer +
      ' / ' +
      model +
      ' / ' +
      systemName +
      ' / ' +
      systemVersion +
      '\n' +
      appLabel +
      '\n' +
      (zingolibVersionEmail
        ? GlobalConst.zingolib + ': ' + zingolibVersionEmail
        : '') +
      '\n\n' +
      bodyEmail,
  )}`;

  try {
    await Linking.openURL(url);
    console.log('Support page opened', SUPPORT_ISSUES_URL);
  } catch (err: unknown) {
    console.log(
      'Error opening support page:',
      err instanceof Error ? err.message : String(err),
    );
    showConfirm({
      title: translate('loadedapp.email-error-title') as string,
      message: translate('loadedapp.email-error-body') as string,
      buttons: [{ text: translate('close') as string }],
    });
  }
};
