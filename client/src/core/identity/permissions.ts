import type { Me } from '../../api/users.io';

/**
 * Capability bitfield, mirrored from `server/src/user/permissions.rs`.
 * Server stores `users.permissions` as one int; we get the same int on
 * `Me.permissions` and check bits with bitwise `&`.
 */
export const Permissions = {
  CAN_AUTHORIZE: 1 << 0,
  MANAGE_TRACKS: 1 << 1,
  MANAGE_USERS: 1 << 2,
  MANAGE_SETTINGS: 1 << 3,
  MANAGE_GLIDERS: 1 << 4,
} as const;

export type Permission = (typeof Permissions)[keyof typeof Permissions];

export const hasPermission = (me: Me, flag: Permission): boolean =>
  (me.permissions & flag) === flag;

/**
 * Bit-level form: any `MANAGE_*` flag set on top of `CAN_AUTHORIZE`.
 * Useful for table cells where we only have the raw integer.
 */
export const isAdminBits = (bits: number): boolean =>
  (bits & ~Permissions.CAN_AUTHORIZE) !== 0;

/**
 * Anyone whose permissions go beyond plain log-in: holds at least one
 * `MANAGE_*` bit. Used to decide whether to surface admin-only UI
 * (e.g. system settings, users list).
 */
export const isAdmin = (me: Me): boolean => isAdminBits(me.permissions);
