import { Divider } from 'antd';
import { getEnabledProviders } from '../../api/oauth';
import { useAsyncData } from '../../core/hooks';
import { OAuthRow } from './OAuthRow';

/**
 * "Sign in with …" picker for enabled providers. Registration via social media
 * is not available yet. Renders nothing until providers load, on error, or when
 * none are enabled.
 */
export function OAuthLoginButtons() {
  const providers = useAsyncData(
    (signal) => getEnabledProviders({ signal }),
    [],
  );

  if (!providers.data || providers.data.length === 0) {
    return null;
  }

  return (
    <>
      <Divider plain>or</Divider>
      <OAuthRow providerIds={providers.data} intent="login" align="center" />
    </>
  );
}
