import { Tooltip } from 'antd';
import clsx from 'clsx';

import type { UserSex } from '../../api/users.io';
import styles from './GenderIcon.module.scss';

export interface GenderIconProps {
  gender: UserSex;
  /** Wrap the glyph in a tooltip showing the gender label. */
  tooltip?: boolean;
  className?: string;
}

/**
 * Coloured gender symbol — blue ♂ / pink ♀ / purple ⚧. Rendered as a
 * text-presentation glyph so the colour comes from CSS, keeping it in
 * step with the surrounding UI rather than the emoji font.
 */
export const GenderIcon = ({
  gender,
  tooltip = false,
  className,
}: GenderIconProps) => {
  const { symbol, label } = GENDER_META[gender];
  const glyph = (
    <span
      className={clsx(styles.icon, styles[gender], className)}
      aria-label={label}
    >
      {symbol}
    </span>
  );

  return tooltip ? <Tooltip title={label}>{glyph}</Tooltip> : glyph;
};

/** Human label for a gender, shared with the form option lists. */
export const genderLabel = (gender: UserSex): string =>
  GENDER_META[gender].label;

const GENDER_META: Record<UserSex, { symbol: string; label: string }> = {
  male: { symbol: '♂', label: 'Male' },
  female: { symbol: '♀', label: 'Female' },
  diverse: { symbol: '⚧', label: 'Diverse' },
};
