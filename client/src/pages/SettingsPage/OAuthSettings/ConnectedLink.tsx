import { CloseOutlined } from '@ant-design/icons';
import { Button, Tooltip } from 'antd';
import type { OAuthLink } from '../../../api/oauth.io';
import { PROVIDER_META } from '../../../features/oauth/providers';
import styles from './ConnectedLink.module.scss';

export function ConnectedLink({ link }: { link: OAuthLink }) {
  const meta = PROVIDER_META[link.provider];

  return (
    <li className={styles.linkItem}>
      <Tooltip title={meta.label}>
        <span className={styles.linkIcon}>
          <meta.Icon />
        </span>
      </Tooltip>
      {link.displayName && (
        <span className={styles.linkName}>{link.displayName}</span>
      )}
      {link.email && <span className={styles.linkEmail}>{link.email}</span>}
      {/* TODO. */}
      <Button
        type="text"
        size="small"
        icon={<CloseOutlined />}
        aria-label={`Unlink ${meta.label}`}
        title="Unlinking isn't available yet"
      />
    </li>
  );
}
