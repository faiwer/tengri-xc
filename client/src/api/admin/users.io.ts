import { z } from 'zod';

import { UserIo as SharedUserIo, UserSexIo, type UserSex } from '../users.io';

/** One row of `GET /admin/users`. Profile-side `country` and `sex` are
 * included for the Name cell; the rest of the profile stays off. */
export const UserListItemIo = z.object({
  id: z.number().int(),
  name: z.string(),
  login: z.string().nullable(),
  email: z.string().nullable(),
  /** Raw `Permissions` bits; see `core/identity/permissions.ts`. */
  permissions: z.number().int(),
  /** ISO 3166-1 alpha-2, or `null` when unset. */
  country: z.string().nullable(),
  /** Self-described gender, or `null` when unset. */
  sex: UserSexIo.nullable(),
  /** Unix epoch seconds (UTC). */
  createdAt: z.number().int(),
  /** Unix epoch seconds (UTC). */
  lastLoginAt: z.number().int().nullable(),
  /** Flights the user owns; shown in the delete-confirm dialog. */
  flightCount: z.number().int(),
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

/**
 * Body for create (`POST /admin/users`) and edit (`PATCH /admin/users/:id`).
 * The form always submits every field, so scalars are a full write; only
 * `password` is special — an empty/omitted value leaves the stored hash
 * alone (and on create means "no password / not yet able to log in").
 */
export interface UserInput {
  name: string;
  login: string | null;
  email: string | null;
  /** Marks/clears `emailVerifiedAt`; an already-verified edit keeps its timestamp. */
  emailVerified: boolean;
  /** Raw `Permissions` bitfield; see `core/identity/permissions.ts`. */
  permissions: number;
  /** Plaintext password to (re)set. `null` keeps the current one unchanged. */
  password: string | null;
  profile: {
    civlId: number | null;
    country: string | null;
    sex: UserSex | null;
  };
}
