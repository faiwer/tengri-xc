import { tengri } from '../support/tengri';

export interface SeededUser {
  login: string;
  name: string;
  email: string;
  /** Left out for accounts that never had one, the way an OAuth sign-up is. */
  password?: string;
  /** `users.permissions` bitfield. Defaults to what the CLI hands a new user. */
  permissions?: number;
}

/**
 * Create a user straight in the database. The address lands in `users.email`,
 * the column only proven addresses reach, so the account behaves like one that
 * already followed its confirmation link.
 */
export async function seedUser(user: SeededUser): Promise<SeededUser> {
  await tengri(
    [
      'user',
      'add',
      '--name',
      user.name,
      '--login',
      user.login,
      '--email',
      user.email,
      ...(user.permissions == null
        ? []
        : ['--permissions', String(user.permissions)]),
      ...(user.password ? ['--password-stdin'] : []),
    ],
    { stdin: user.password },
  );

  return user;
}
