import { apiGet, apiPatch, type ApiRequestOptions } from '../core';
import {
  AdminOAuthProviderListIo,
  type AdminOAuthProvider,
  type OAuthProviderId,
  type UpdateOAuthProviderRequest,
} from './oauthProviders.io';

/**
 * `GET /admin/oauth-providers` — every configured provider (0..5). Unconfigured
 * providers are absent; the caller merges against {@link OAUTH_PROVIDERS}.
 */
export async function getAdminOAuthProviders(
  options: ApiRequestOptions = {},
): Promise<AdminOAuthProvider[]> {
  return apiGet('/admin/oauth-providers', AdminOAuthProviderListIo, options);
}

/**
 * `PATCH /admin/oauth-providers/:provider` — partial update. Returns the full
 * refreshed list so the caller can re-seed the edited row (and any others).
 *
 * On 422, throws `ValidationError` carrying `clientId` / `clientSecret`
 * messages that land straight on the form fields.
 */
export async function updateAdminOAuthProvider(
  provider: OAuthProviderId,
  body: UpdateOAuthProviderRequest,
  options: ApiRequestOptions = {},
): Promise<AdminOAuthProvider[]> {
  return apiPatch(
    `/admin/oauth-providers/${provider}`,
    body,
    AdminOAuthProviderListIo,
    options,
  );
}
