import {
  apiGet,
  apiPatch,
  apiPost,
  apiPostVoid,
  type ApiRequestOptions,
} from './core';
import {
  MeIo,
  MeResponseIo,
  UpdateMeResponseIo,
  type ChangePasswordRequest,
  type Me,
  type RegisterRequest,
  type UpdateMeRequest,
  type UpdateMeResponse,
} from './users.io';

export interface LoginParams {
  /** `login` or `email`, case-insensitive. */
  identifier: string;
  password: string;
}

/**
 * `POST /users/login`. On success the server sets the `tengri-jwt`
 * cookie and echoes the same body shape as `getMe()`. The cookie is
 * `HttpOnly`, so we keep the user state ourselves via the returned
 * value.
 */
export async function login(params: LoginParams): Promise<Me> {
  return apiPost('/users/login', params, MeIo);
}

/**
 * `POST /users/register` — 204, and no session: the account can't sign in until
 * the confirmation link in its inbox has been followed. Following that link is
 * what logs the user in.
 *
 * On 422, throws [`ValidationError`] with per-field messages keyed `name` /
 * `login` / `email` / `password`. 403 means registration is switched off
 * site-wide, 409 that the server can't send the confirmation mail.
 */
export const register = async (
  body: RegisterRequest,
  options: ApiRequestOptions = {},
): Promise<void> => apiPostVoid('/users/register', body, options);

/** `POST /users/logout` — clears the cookie. Always 204. */
export async function logout(): Promise<void> {
  return apiPostVoid('/users/logout');
}

/**
 * `GET /users/me` — `null` for anonymous (or a user whose row was
 * deleted / had `CAN_AUTHORIZE` revoked while the token was live).
 */
export async function getMe(
  options: ApiRequestOptions = {},
): Promise<Me | null> {
  return apiGet('/users/me', MeResponseIo, options);
}

/**
 * `PATCH /users/me` — owner-self update for any subset of editable sections
 * (currently `profile` and `preferences`). Returns the full updated {@link Me}
 * (so the caller can swap it into the identity context wholesale) plus
 * `confirmationSentTo`, set when the request mailed a confirmation link instead
 * of writing the new address.
 *
 * On 422, throws {@link ValidationError} (from `core`) carrying the per-field
 * messages. On 409, outgoing mail isn't configured, so an email change can't be
 * confirmed and the whole save is refused.
 */
export const updateMe = async (
  body: UpdateMeRequest,
  options: ApiRequestOptions = {},
): Promise<UpdateMeResponse> =>
  apiPatch('/users/me', body, UpdateMeResponseIo, options);

/**
 * `POST /users/me/password` — owner-self password change (and initial
 * `login` set for accounts that have none). Returns the refreshed
 * [`Me`] so the caller can swap it into the identity context.
 *
 * On 422, throws [`ValidationError`] with per-field messages keyed
 * `login` / `currentPassword` / `newPassword`.
 */
export const changeMyPassword = async (
  body: ChangePasswordRequest,
  options: ApiRequestOptions = {},
): Promise<Me> => apiPost('/users/me/password', body, MeIo, options);
