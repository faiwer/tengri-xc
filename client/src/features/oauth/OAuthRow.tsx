import { Tooltip } from 'antd';
import clsx from 'clsx';
import type { OAuthProviderId } from '../../api/admin/oauthProviders.io';
import { startOAuth, type OAuthIntent } from '../../api/oauth';
import { PROVIDER_META } from './providers';
import styles from './OAuthRow.module.scss';

interface OAuthRowProps {
  providerIds: OAuthProviderId[];
  intent: OAuthIntent;
  align?: 'left' | 'center';
}

/** A single row of provider icon buttons, each starting an OAuth flow. */
export function OAuthRow({ providerIds, intent, align = 'left' }: OAuthRowProps) {
  return (
    <div className={clsx(styles.row, align === 'center' && styles.center)}>
      {providerIds.map((id) => {
        const meta = PROVIDER_META[id];
        const label =
          intent === 'login' ? `Sign in with ${meta.label}` : `Link ${meta.label}`;
        return (
          <Tooltip key={id} title={label}>
            <button
              type="button"
              className={styles.oauthButton}
              aria-label={label}
              onClick={() => startOAuth(id, intent)}
            >
              <meta.Icon />
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
}
