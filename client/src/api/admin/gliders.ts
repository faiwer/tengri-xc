import { apiGet, type ApiRequestOptions } from '../core';
import { GliderCatalogIo, type Sport, type GliderCatalog } from './gliders.io';

/**
 * `GET /admin/gliders?kind=...` — the full brand + model catalog for one sport.
 * Each sport is treated as a separate source: switching sports in the UI
 * triggers a fresh request rather than filtering a single catalog. Requires
 * `MANAGE_GLIDERS`.
 */
export const getGliderCatalog = (
  sport: Sport,
  options: ApiRequestOptions = {},
): Promise<GliderCatalog> =>
  apiGet('/admin/gliders', GliderCatalogIo, {
    ...options,
    query: { kind: sport, ...options.query },
  });
