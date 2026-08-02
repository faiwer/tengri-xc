import { apiGet, type ApiRequestOptions } from '../core';
import { UserIo, UsersPageIo, type User, type UsersPage } from './users.io';

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
