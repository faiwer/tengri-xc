import { z } from 'zod';

import { MeIo } from '../users.io';

/** One row of `GET /admin/users`. Trimmed projection — no profile join. */
export const UserListItemIo = z.object({
  id: z.number().int(),
  name: z.string(),
  login: z.string().nullable(),
  email: z.string().nullable(),
  /** Raw `Permissions` bits; see `core/identity/permissions.ts`. */
  permissions: z.number().int(),
  /** Unix epoch seconds (UTC). */
  createdAt: z.number().int(),
  /** Unix epoch seconds (UTC). */
  lastLoginAt: z.number().int().nullable(),
});

export type UserListItem = z.infer<typeof UserListItemIo>;

export const UsersPageIo = z.object({
  items: z.array(UserListItemIo),
  /** Opaque cursor for the next page; `null` on the last page. */
  nextCursor: z.string().nullable(),
});

export type UsersPage = z.infer<typeof UsersPageIo>;

/**
 * `GET /admin/users/:id` returns the same shape as `/users/me` — the
 * server's `UserDto`. Re-export `MeIo` rather than re-declare it so a
 * field added on the server only needs one schema update on the client.
 */
export const UserIo = MeIo;
export type User = z.infer<typeof UserIo>;
