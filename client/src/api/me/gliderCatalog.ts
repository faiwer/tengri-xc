import {
  GliderCatalogIo,
  type GliderCatalog,
  type Sport,
} from '../admin/gliders.io';
import { apiGet, type ApiRequestOptions } from '../core';

/**
 * `GET /me/gliders/catalog?kind=...` — brand + model catalog for one sport,
 * scoped to what the caller may pick: canonical rows plus their own customs.
 * Session-only sibling of the admin catalog; used by the upload flow's glider
 * pickers, which normal pilots reach without `MANAGE_GLIDERS`.
 */
export const getMyGliderCatalog = (
  sport: Sport,
  options: ApiRequestOptions = {},
): Promise<GliderCatalog> =>
  apiGet('/me/gliders/catalog', GliderCatalogIo, {
    ...options,
    query: { kind: sport, ...options.query },
  });
