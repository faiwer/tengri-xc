import { Spin, Typography } from 'antd';
import { useState } from 'react';
import { getEnabledProviders, getMyLinks } from '../../../api/oauth';
import { useAsyncData, useErrorToast } from '../../../core/hooks';
import { useIdentity } from '../../../core/identity';
import { OAuthRow } from '../../../features/oauth/OAuthRow';
import { ConnectedLink } from './ConnectedLink';
import styles from './LinkedAccounts.module.scss';

/**
 * The caller's connected accounts as an ordered list, plus a row of enabled
 * providers to start a new link flow. Linking is a full-page redirect, so a
 * return from the provider remounts this and refetches; unlinking refetches
 * in place via `reloadKey`.
 */
export function LinkedAccounts() {
  const { me } = useIdentity();
  const [reloadKey, setReloadKey] = useState(0);

  const providers = useAsyncData(
    (signal) => getEnabledProviders({ signal }),
    [],
  );
  const links = useAsyncData((signal) => getMyLinks({ signal }), [reloadKey]);

  useErrorToast(providers.error ?? links.error, {
    title: "Couldn't load social connections",
  });

  if (providers.isLoading || links.isLoading) {
    return <Spin size="small" />;
  }

  if (providers.error || links.error) {
    return null;
  }

  if (providers.data.length === 0) {
    return (
      <Typography.Text type="secondary">
        Authorization via social media is not available on this platform.
      </Typography.Text>
    );
  }

  // Unlinking the sole sign-in method would brick a password-less account, so
  // lock the last link's remove button in that case (the server enforces this
  // too).
  const lockLastLink = !me?.hasPassword && links.data.length === 1;

  return (
    <div className={styles.container}>
      {links.data.length > 0 && (
        <div className={styles.section}>
          <h4 className={styles.sectionHeader}>Connected accounts</h4>
          <ol className={styles.linkList}>
            {links.data.map((link) => (
              <ConnectedLink
                key={`${link.provider}:${link.providerUserId}`}
                link={link}
                onUnlinked={() => setReloadKey((k) => k + 1)}
                locked={lockLastLink}
              />
            ))}
          </ol>
        </div>
      )}

      <div className={styles.section}>
        <h4 className={styles.sectionHeader}>Add a new account</h4>
        <OAuthRow providerIds={providers.data} intent="link" />
      </div>
    </div>
  );
}
