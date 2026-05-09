import clsx from 'clsx';

import { Permissions } from '../../core/identity';
import styles from './PermissionBadges.module.scss';

type Variant = 'users' | 'tracks' | 'settings' | 'login';

interface BadgeDef {
  flag: number;
  /** Long form, used in the user-detail view. */
  label: string;
  /** Short form, used in the dense table cell. */
  short: string;
  variant: Variant;
}

const VARIANT_CLASS: Record<Variant, string> = {
  users: styles.variantUsers,
  tracks: styles.variantTracks,
  settings: styles.variantSettings,
  login: styles.variantLogin,
};

const BADGES: BadgeDef[] = [
  {
    flag: Permissions.MANAGE_USERS,
    label: 'manage users',
    short: 'users',
    variant: 'users',
  },
  {
    flag: Permissions.MANAGE_TRACKS,
    label: 'manage tracks',
    short: 'tracks',
    variant: 'tracks',
  },
  {
    flag: Permissions.MANAGE_SETTINGS,
    label: 'manage settings',
    short: 'settings',
    variant: 'settings',
  },
  // `CAN_AUTHORIZE` last: it's the universal default and rarely the
  // interesting bit on an admin row.
  {
    flag: Permissions.CAN_AUTHORIZE,
    label: 'can log in',
    short: 'login',
    variant: 'login',
  },
];

export interface PermissionBadgesProps {
  bits: number;
  /** Use the short label set; sensible default for tight table cells. */
  compact?: boolean;
}

/** Renders the set bits in `Permissions` as small coloured badges. */
export function PermissionBadges({
  bits,
  compact = false,
}: PermissionBadgesProps) {
  const matched = BADGES.filter((b) => (bits & b.flag) === b.flag);
  if (matched.length === 0) {
    return <span className={styles.empty}>none</span>;
  }
  return (
    <span className={styles.row}>
      {matched.map((b) => (
        <span
          key={b.flag}
          className={clsx(styles.badge, VARIANT_CLASS[b.variant])}
        >
          {compact ? b.short : b.label}
        </span>
      ))}
    </span>
  );
}
