import { z } from 'zod';
import { AdminOAuthProviderIo } from './admin/oauthProviders.io';

/**
 * The `provider` discriminant, reused from the admin schema so the login/link
 * surface and the credential editor can't drift apart on the provider set.
 */
export const OAuthProviderIdIo = AdminOAuthProviderIo.shape.provider;

/** `GET /oauth/providers` — the enabled providers offered for login/link. */
export const EnabledProvidersIo = z.array(OAuthProviderIdIo);

/**
 * One connected account from `GET /oauth/links`. `email` / `displayName` are
 * snapshots captured at link time and may be absent (X returns no email; some
 * providers omit a name).
 */
export const OAuthLinkIo = z.object({
  provider: OAuthProviderIdIo,
  providerUserId: z.string(),
  email: z.string().nullable(),
  displayName: z.string().nullable(),
});

export type OAuthLink = z.infer<typeof OAuthLinkIo>;

export const OAuthLinkListIo = z.array(OAuthLinkIo);
