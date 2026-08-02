import { apiGet, type ApiRequestOptions } from '../core';
import { SitesPageIo, type SitesPage } from './sites.io';

export interface GetSitesPageParams extends ApiRequestOptions {
  /** Case-insensitive substring match on `name`. */
  q?: string;
  /** Pass through the `nextCursor` from the previous page. */
  cursor?: string;
  /** Server caps at 100; defaults to 50 when omitted. */
  limit?: number;
}

/** `GET /admin/sites` — paginated sites list. Requires `MANAGE_SITES`. */
export const getSitesPage = ({
  q,
  cursor,
  limit,
  signal,
}: GetSitesPageParams = {}): Promise<SitesPage> =>
  apiGet('/admin/sites', SitesPageIo, { signal, query: { q, cursor, limit } });
