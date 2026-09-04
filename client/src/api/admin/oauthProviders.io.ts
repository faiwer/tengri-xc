import { z } from 'zod';

/**
 * Full per-provider config from `GET /admin/oauth-providers` and the list
 * returned by `PATCH /admin/oauth-providers/:provider`. The endpoint returns
 * only *configured* providers (rows that exist), and a row exists only once
 * both credentials are set — so `clientId` / `clientSecret` are non-null. The
 * secret is included because the editor prefills its field from it.
 */
/**
 * Who a configured provider is offered to: `disabled` hides it, `admins` offers
 * it only to callers with `MANAGE_USERS`, `public` offers it to everyone.
 */
export const OAuthVisibilityIo = z.enum(['disabled', 'admins', 'public']);

export type OAuthVisibility = z.infer<typeof OAuthVisibilityIo>;

export const AdminOAuthProviderIo = z.object({
  provider: z.enum(['google', 'facebook', 'x', 'microsoft', 'github']),
  clientId: z.string(),
  clientSecret: z.string(),
  visibility: OAuthVisibilityIo,
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
  visibility?: OAuthVisibility;
}
