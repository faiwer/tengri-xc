import { apiGet, apiPatch, apiPostVoid, type ApiRequestOptions } from '../core';
import {
  AdminSiteIo,
  type AdminSite,
  type SendTestEmailRequest,
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

/**
 * `POST /admin/site/test-email` — send one message using the settings in the
 * body rather than the stored ones, so a connection can be verified before
 * it's saved. Always 204.
 *
 * On 422, throws `ValidationError` carrying `to`. A missing host or
 * from-address, and any SMTP failure, arrive as `HttpError` with a message
 * worth showing — the point of the button is the reason it failed.
 */
export const sendTestEmail = (
  body: SendTestEmailRequest,
  options: ApiRequestOptions = {},
): Promise<void> => apiPostVoid('/admin/site/test-email', body, options);
