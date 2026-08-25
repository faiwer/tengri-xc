import { CloseOutlined } from '@ant-design/icons';
import { Button, Popconfirm, Tooltip } from 'antd';
import { unlinkOAuth } from '../../../api/oauth';
import type { OAuthLink } from '../../../api/oauth.io';
import { useAsync, useErrorToast } from '../../../core/hooks';
import { PROVIDER_META } from '../../../features/oauth/providers';
import styles from './ConnectedLink.module.scss';

interface ConnectedLinkProps {
  link: OAuthLink;
  onUnlinked: () => void;
  /** Disable unlinking — this is the account's only remaining sign-in method. */
  locked?: boolean;
}

export function ConnectedLink({
  link,
  onUnlinked,
  locked,
}: ConnectedLinkProps) {
  const meta = PROVIDER_META[link.provider];

  const [unlink, isUnlinking, error] = useAsync(async () => {
    await unlinkOAuth(link.provider, link.providerUserId);
    onUnlinked();
  });

  useErrorToast(error, { title: "Couldn't unlink account" });

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
      {locked ? (
        <Tooltip title="Set a password before unlinking your only sign-in method.">
          {/* Span wrapper so the tooltip fires over the disabled button. */}
          <span>
            <Button
              type="text"
              size="small"
              icon={<CloseOutlined />}
              aria-label={`Unlink ${meta.label}`}
              disabled
            />
          </span>
        </Tooltip>
      ) : (
        <Popconfirm
          title="Unlink this account?"
          okText="Unlink"
          okButtonProps={{ danger: true }}
          onConfirm={() => unlink().catch(() => {})}
        >
          <Button
            type="text"
            size="small"
            icon={<CloseOutlined />}
            aria-label={`Unlink ${meta.label}`}
            loading={isUnlinking}
          />
        </Popconfirm>
      )}
    </li>
  );
}
