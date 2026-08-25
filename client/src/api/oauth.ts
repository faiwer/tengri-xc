import type { OAuthProviderId } from './admin/oauthProviders.io';
import { apiDelete, apiGet, type ApiRequestOptions } from './core';
import {
  EnabledProvidersIo,
  OAuthLinkListIo,
  type OAuthLink,
} from './oauth.io';

const SERVER_URL = import.meta.env.VITE_SERVER_URL;

/** `GET /oauth/providers` — enabled providers, for the login picker + link UI. */
export async function getEnabledProviders(
  options: ApiRequestOptions = {},
): Promise<OAuthProviderId[]> {
  return apiGet('/oauth/providers', EnabledProvidersIo, options);
}

/** `GET /oauth/links` — the caller's connected accounts. Requires a session. */
export async function getMyLinks(
  options: ApiRequestOptions = {},
): Promise<OAuthLink[]> {
  return apiGet('/oauth/links', OAuthLinkListIo, options);
}

/** `DELETE /oauth/links/:provider/:providerUserId` — unlink a connected account. */
export async function unlinkOAuth(
  provider: OAuthProviderId,
  providerUserId: string,
  options: ApiRequestOptions = {},
): Promise<void> {
  return apiDelete(
    `/oauth/links/${provider}/${encodeURIComponent(providerUserId)}`,
    options,
  );
}

/** Why we're starting a flow. `link` needs a session; `login` is anonymous. */
export type OAuthIntent = 'login' | 'link';

/**
 * Send the browser (top-level) into the provider's authorize flow. This is a
 * full navigation, not a fetch: the server 302s to the provider and eventually
 * back to `${APP_BASE_URL}${returnTo}?oauth=…`, so it never resolves here.
 *
 * `returnTo` defaults to the current SPA path so the user lands back where they
 * started (the server sanitizes it against open-redirects).
 */
export function startOAuth(
  provider: OAuthProviderId,
  intent: OAuthIntent,
  returnTo: string = window.location.pathname + window.location.search,
): void {
  const params = new URLSearchParams({ intent, return_to: returnTo });
  window.location.assign(`${SERVER_URL}/oauth/${provider}/start?${params}`);
}
