import {
  apiDelete,
  apiGet,
  apiPatch,
  apiPost,
  type ApiRequestOptions,
} from '../core';
import {
  ReindexResultIo,
  SiteListItemIo,
  SitesPageIo,
  type ReindexResult,
  type SiteInput,
  type SiteListItem,
  type SitesPage,
} from './sites.io';

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

/** `POST /admin/sites` — create a site. Requires `MANAGE_SITES`. */
export const createSite = (
  input: SiteInput,
  options: ApiRequestOptions = {},
): Promise<SiteListItem> =>
  apiPost('/admin/sites', input, SiteListItemIo, options);

/** `PATCH /admin/sites/:id` — full replace of a site. Requires `MANAGE_SITES`. */
export const updateSite = (
  id: number,
  input: SiteInput,
  options: ApiRequestOptions = {},
): Promise<SiteListItem> =>
  apiPatch(`/admin/sites/${id}`, input, SiteListItemIo, options);

/** `DELETE /admin/sites/:id`. Requires `MANAGE_SITES`. */
export const deleteSite = (
  id: number,
  options: ApiRequestOptions = {},
): Promise<void> => apiDelete(`/admin/sites/${id}`, options);

/**
 * `POST /admin/sites/reindex` — recompute every flight's closest takeoff site.
 * Requires `MANAGE_SITES`.
 */
export const reindexSites = (
  options: ApiRequestOptions = {},
): Promise<ReindexResult> =>
  apiPost('/admin/sites/reindex', null, ReindexResultIo, options);
