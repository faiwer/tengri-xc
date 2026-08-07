import { Skeleton } from 'antd';
import { Navigate } from 'react-router';
import { getAdminOAuthProviders } from '../../../api/admin/oauthProviders';
import { OAUTH_PROVIDERS } from '../../../features/oauth/providers';
import { LoadError } from '../../../components/LoadError';
import { useAsyncData } from '../../../core/hooks';
import {
  hasPermission,
  Permissions,
  useIdentity,
} from '../../../core/identity';
import { routes } from '../../../core/routes';
import { SettingsSection } from '../SettingsSection';
import { ProviderRow } from './ProviderRow';

/**
 * Admin editor at `/settings/oauth-providers`. Same two-layer gate as
 * {@link SystemSettings}: identity loads first (skeleton / redirect anonymous
 * to login / redirect non-`MANAGE_SETTINGS` viewers to their profile), then the
 * per-provider config fetches on mount.
 */
export function AppOAuthSettings() {
  const { me, isLoading, error, retry } = useIdentity();

  if (isLoading) {
    return <Skeleton active paragraph={{ rows: 8 }} />;
  }

  if (error) {
    return (
      <LoadError
        title="Couldn't load your account"
        error={error}
        onRetry={retry}
      />
    );
  }

  if (!me) {
    return <Navigate replace to={routes.login()} />;
  }

  if (!hasPermission(me, Permissions.MANAGE_SETTINGS)) {
    return <Navigate replace to={routes.settings.profile()} />;
  }

  return <AppOAuthSettingsLoader />;
}

function AppOAuthSettingsLoader() {
  const config = useAsyncData(
    (signal) => getAdminOAuthProviders({ signal }),
    [],
  );

  if (config.error) {
    return <LoadError title="Couldn't load OAuth settings" error={config.error} />;
  }

  if (config.isLoading) {
    return <Skeleton active paragraph={{ rows: 10 }} />;
  }

  return (
    <SettingsSection
      title="OAuth"
      subtitle="Client credentials per provider. A provider only goes live for login once it's fully configured and enabled."
      scrollable
    >
      {OAUTH_PROVIDERS.map((provider) => (
        <ProviderRow
          key={provider.id}
          provider={provider}
          config={config.data.find((c) => c.provider === provider.id) ?? null}
        />
      ))}
    </SettingsSection>
  );
}
