import { Spin, Typography } from 'antd';
import { getEnabledProviders, getMyLinks } from '../../../api/oauth';
import { useAsyncData, useErrorToast } from '../../../core/hooks';
import { OAuthRow } from '../../../features/oauth/OAuthRow';
import { ConnectedLink } from './ConnectedLink';
import styles from './LinkedAccounts.module.scss';

/**
 * The caller's connected accounts as an ordered list, plus a row of enabled
 * providers to start a new link flow. Linking is a full-page redirect, so a
 * return from the provider remounts this and refetches — no in-place refresh.
 */
export function LinkedAccounts() {
  const providers = useAsyncData(
    (signal) => getEnabledProviders({ signal }),
    [],
  );
  const links = useAsyncData((signal) => getMyLinks({ signal }), []);

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

  return (
    <div className={styles.container}>
      {links.data.length > 0 && (
        <div className={styles.section}>
          <h4 className={styles.sectionHeader}>Connected accounts</h4>
          <ol className={styles.linkList}>
            {links.data.map((link, index) => (
              <ConnectedLink key={index} link={link} />
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
