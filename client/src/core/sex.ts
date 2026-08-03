import type { UserSex } from '../api/users.io';

/**
 * Sex options for form controls, ordered for display. Single source
 * shared by the profile form and the admin user form.
 */
export const SEX_OPTIONS: { label: string; value: UserSex }[] = [
  { label: 'Male', value: 'male' },
  { label: 'Female', value: 'female' },
  { label: 'Diverse', value: 'diverse' },
];
