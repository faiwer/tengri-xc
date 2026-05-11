import { apiGet, apiPatch, type ApiRequestOptions } from '../core';
import {
  AdminSiteIo,
  type AdminSite,
  type UpdateAdminSiteRequest,
} from './site.io';

/** `GET /admin/site` — full state for the operator editor. */
export async function getAdminSite(
  options: ApiRequestOptions = {},
): Promise<AdminSite> {
  return apiGet('/admin/site', AdminSiteIo, options);
}

/**
 * `PATCH /admin/site` — partial update. Returns the full updated state so the
 * form can refresh its values and the caller can derive the slim public shape
 * to refresh the `useSite()` context.
 *
 * On 422, throws `ValidationError` (from `core`) carrying the per-field
 * messages (`siteName`, `tosMd`, `privacyMd`).
 */
export async function updateAdminSite(
  body: UpdateAdminSiteRequest,
  options: ApiRequestOptions = {},
): Promise<AdminSite> {
  return apiPatch('/admin/site', body, AdminSiteIo, options);
}
