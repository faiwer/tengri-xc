import type { ReactNode } from 'react';

import type { UserSex } from '../api/users.io';
import { GenderIcon, genderLabel } from '../components/GenderIcon';
import styles from './sex.module.scss';

/**
 * Sex options for form controls, ordered for display. Single source
 * shared by the profile form and the admin user form. Each label pairs
 * the coloured {@link GenderIcon} with its text so the segmented profile
 * control and the admin select stay in step.
 */
export const SEX_OPTIONS: { label: ReactNode; value: UserSex }[] = (
  ['male', 'female', 'diverse'] as const
).map((value) => ({
  value,
  label: (
    <span className={styles.option}>
      {genderLabel(value)}
      <GenderIcon gender={value} />
    </span>
  ),
}));
