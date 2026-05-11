import { z } from 'zod';

import { UserIo as SharedUserIo } from '../users.io';

/** One row of `GET /admin/users`. Profile-side `country` is included
 * for the flag in the Name cell; the rest of the profile stays off. */
export const UserListItemIo = z.object({
  id: z.number().int(),
  name: z.string(),
  login: z.string().nullable(),
  email: z.string().nullable(),
  /** Raw `Permissions` bits; see `core/identity/permissions.ts`. */
  permissions: z.number().int(),
  /** ISO 3166-1 alpha-2, or `null` when unset. */
  country: z.string().nullable(),
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
 * `GET /admin/users/:id` returns the server's `UserDto` — the same
 * base record that `/users/me` extends with preferences. We re-export
 * the shared schema rather than re-declare it so a field added on the
 * server only needs one client-side schema change.
 */
export const UserIo = SharedUserIo;
export type User = z.infer<typeof UserIo>;
