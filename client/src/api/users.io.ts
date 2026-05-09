import { z } from 'zod';

// Wire JSON is snake_case; `apiGet`/`apiPost` camelize at the
// boundary, so describe the post-conversion shape here.

const UserSourceIo = z.enum(['internal', 'leo']);
export type UserSource = z.infer<typeof UserSourceIo>;

const UserSexIo = z.enum(['male', 'female', 'diverse']);
export type UserSex = z.infer<typeof UserSexIo>;

const MeProfileIo = z.object({
  civlId: z.number().int().nullable(),
  country: z.string().nullable(),
  sex: UserSexIo.nullable(),
});

export const MeIo = z.object({
  id: z.number().int(),
  name: z.string(),
  login: z.string().nullable(),
  email: z.string().nullable(),
  source: UserSourceIo,
  /** Raw bits — `permissions & N` checks; mirrors `Permissions` on the server. */
  permissions: z.number().int(),
  /** Unix epoch seconds (UTC). Convert with `new Date(value * 1000)`. */
  emailVerifiedAt: z.number().int().nullable(),
  /** Unix epoch seconds (UTC). */
  lastLoginAt: z.number().int().nullable(),
  /** Unix epoch seconds (UTC). */
  createdAt: z.number().int(),
  profile: MeProfileIo.nullable(),
});

export type Me = z.infer<typeof MeIo>;

/** `/users/me` returns the user or `null` for anonymous (always 200). */
export const MeResponseIo = MeIo.nullable();
