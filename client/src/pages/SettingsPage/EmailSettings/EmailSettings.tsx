import { Skeleton } from 'antd';
import { useState } from 'react';
import { Navigate } from 'react-router';
import { getAdminSite } from '../../../api/admin/site';
import type { AdminSite } from '../../../api/admin/site.io';
import { LoadError } from '../../../components/LoadError';
import { useAsyncEffect, useEventHandler } from '../../../core/hooks';
import {
  hasPermission,
  Permissions,
  useIdentity,
} from '../../../core/identity';
import { routes } from '../../../core/routes';
import { EmailSettingsForm } from './EmailSettingsForm';

/**
 * Operator editor at `/settings/email`. Same two-layer gate and the same
 * `GET`/`PATCH /admin/site` endpoints as {@link SystemSettings} — the mail
 * columns live on the same singleton row, they just get their own page because
 * the connection settings have nothing to do with site branding.
 */
export function EmailSettings() {
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

  return <EmailSettingsLoader />;
}

function EmailSettingsLoader() {
  const [initial, setInitial] = useState<AdminSite | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [retryToken, setRetryToken] = useState(0);

  useAsyncEffect(
    async (signal) => {
      setError(null);
      try {
        const next = await getAdminSite({ signal });
        if (!signal.aborted) setInitial(next);
      } catch (err) {
        if (!signal.aborted) setError(err);
      }
    },
    [retryToken],
  );

  const retry = useEventHandler(() => {
    setInitial(null);
    setRetryToken((t) => t + 1);
  });

  if (error && initial === null) {
    return (
      <LoadError
        title="Couldn't load email settings"
        error={error}
        onRetry={retry}
      />
    );
  }

  if (!initial) {
    return <Skeleton active paragraph={{ rows: 10 }} />;
  }

  return <EmailSettingsForm initial={initial} />;
}
