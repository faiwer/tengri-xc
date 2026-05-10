import type { Preferences } from '../../api/users.io';

export type ResolvedPreferences = {
  [K in keyof Preferences]: Exclude<Preferences[K], 'system'>;
};
