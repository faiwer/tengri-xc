import { apiGet, HttpError, type ApiRequestOptions } from './core';
import { SiteIo, SiteDocumentIo, type DocKind, type Site } from './site.io';

/**
 * `GET /site` — slim public payload, loaded once at app boot. Always 200 (the
 * row is guaranteed by the migration).
 */
export async function getSite(options: ApiRequestOptions = {}): Promise<Site> {
  return apiGet('/site', SiteIo, options);
}

/**
 * `GET /site/{kind}` — long-form ToS / Privacy markdown, or `null` when the
 * column is NULL ("not yet published"). 404 is the idiomatic shape there; we
 * translate it to `null` so callers don't branch on an exception for an
 * expected state.
 */
export async function getSiteDocument(
  kind: DocKind,
  options: ApiRequestOptions = {},
): Promise<string | null> {
  try {
    const { md } = await apiGet(`/site/${kind}`, SiteDocumentIo, options);
    return md;
  } catch (err) {
    if (err instanceof HttpError && err.status === 404) return null;
    throw err;
  }
}
