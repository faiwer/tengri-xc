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

export type MeProfile = z.infer<typeof MeProfileIo>;

/**
 * Raw, wire-shaped preferences. Each field is the literal saved by
 * the server — `'system'` is a sentinel meaning "follow the user's
 * locale", resolved to a concrete value by `PreferencesProvider`.
 *
 * Don't read this directly in render code; consume `usePreferences()`
 * instead (which returns the resolved values). The settings UI does
 * read this directly so the user can see "System" as a chosen option.
 */
export const PreferencesIo = z.object({
  timeFormat: z.enum(['system', 'h12', 'h24']),
  dateFormat: z.enum(['system', 'dmy', 'mdy']),
  /** Drives both altitude (m vs ft) and XC distance (km vs mi). */
  units: z.enum(['system', 'metric', 'imperial']),
  /** Independent of `units` — instrument-driven hybrids exist. */
  varioUnit: z.enum(['system', 'mps', 'fpm']),
  speedUnit: z.enum(['system', 'kmh', 'mph']),
  weekStart: z.enum(['system', 'mon', 'sun']),
});

export type Preferences = z.infer<typeof PreferencesIo>;

/**
 * Wire shape shared by `/users/me` and `/admin/users/:id`. Mirrors
 * the server's `UserDto`. The `/users/me` payload extends this with
 * a `preferences` block — see {@link MeIo}.
 */
export const UserIo = z.object({
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

export type User = z.infer<typeof UserIo>;

/**
 * `/users/me` payload: the user record plus their preferences. Admin
 * endpoints return {@link UserIo} without preferences — they're private
 * to the owning user and have no business in management views.
 */
export const MeIo = UserIo.extend({
  /**
   * Always present for any logged-in user — the server's
   * `user_preferences` row is created eagerly via trigger when the
   * user is inserted.
   */
  preferences: PreferencesIo,
});

export type Me = z.infer<typeof MeIo>;

/** `/users/me` returns the user or `null` for anonymous (always 200). */
export const MeResponseIo = MeIo.nullable();

/**
 * Partial of {@link Preferences} — every field optional, same value union.
 * Derived from the schema so the wire literals stay defined in exactly
 * one place ({@link PreferencesIo}).
 */
export type UpdatePreferencesRequest = Partial<Preferences>;

/**
 * Partial of {@link MeProfile} — every field optional, same nullable
 * value type. `null` clears the column, omitting leaves it alone.
 * JS doesn't distinguish absent vs. `undefined`, so we filter
 * undefined fields out before serialisation.
 */
export type UpdateProfileRequest = Partial<MeProfile>;

export interface UpdateMeRequest {
  profile?: UpdateProfileRequest;
  preferences?: UpdatePreferencesRequest;
}
