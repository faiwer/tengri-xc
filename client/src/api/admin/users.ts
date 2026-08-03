import { apiGet, apiPatch, apiPost, type ApiRequestOptions } from '../core';
import {
  UserIo,
  UsersPageIo,
  type UserInput,
  type User,
  type UsersPage,
} from './users.io';

export interface GetUsersPageParams extends ApiRequestOptions {
  /** Case-insensitive substring match on `name` and `email`. */
  q?: string;
  /** Pass through the `nextCursor` from the previous page. */
  cursor?: string;
  /** Server caps at 100; defaults to 25 when omitted. */
  limit?: number;
}

/** `GET /admin/users` — paginated user list. Requires `MANAGE_USERS`. */
export const getUsersPage = ({
  q,
  cursor,
  limit,
  signal,
}: GetUsersPageParams = {}): Promise<UsersPage> =>
  apiGet('/admin/users', UsersPageIo, { signal, query: { q, cursor, limit } });

/** `GET /admin/users/:id` — full user record. Requires `MANAGE_USERS`. */
export const getUser = (
  id: number,
  options: ApiRequestOptions = {},
): Promise<User> => apiGet(`/admin/users/${id}`, UserIo, options);

/** `POST /admin/users` — create an internal user. Requires `MANAGE_USERS`. */
export const createUser = (
  input: UserInput,
  options: ApiRequestOptions = {},
): Promise<User> => apiPost('/admin/users', input, UserIo, options);

/** `PATCH /admin/users/:id` — edit an existing user. Requires `MANAGE_USERS`. */
export const updateUser = (
  id: number,
  input: UserInput,
  options: ApiRequestOptions = {},
): Promise<User> => apiPatch(`/admin/users/${id}`, input, UserIo, options);
