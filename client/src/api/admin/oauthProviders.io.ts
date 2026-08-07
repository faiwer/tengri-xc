import { z } from 'zod';

/**
 * Full per-provider config from `GET /admin/oauth-providers` and the list
 * returned by `PATCH /admin/oauth-providers/:provider`. The endpoint returns
 * only *configured* providers (rows that exist), and a row exists only once
 * both credentials are set — so `clientId` / `clientSecret` are non-null. The
 * secret is included because the editor prefills its field from it.
 */
export const AdminOAuthProviderIo = z.object({
  provider: z.enum(['google', 'facebook', 'x', 'microsoft', 'github']),
  clientId: z.string(),
  clientSecret: z.string(),
  enabled: z.boolean(),
});

export type AdminOAuthProvider = z.infer<typeof AdminOAuthProviderIo>;

/** `provider` discriminant value, e.g. `'google'`. */
export type OAuthProviderId = AdminOAuthProvider['provider'];

export const AdminOAuthProviderListIo = z.array(AdminOAuthProviderIo);

/**
 * Partial update. Omit a field to leave it untouched; an empty-string
 * credential is also treated as "unchanged" by the server (the columns are NOT
 * NULL, so there's no "clear to empty").
 */
export interface UpdateOAuthProviderRequest {
  clientId?: string;
  clientSecret?: string;
  enabled?: boolean;
}
