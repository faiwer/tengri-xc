import { tengri } from '../support/tengri';

/**
 * Provider the suite runs its OAuth flows against. Google's userinfo answers
 * with a `sub`, a name, and an address it either vouches for or doesn't, all
 * in one call — enough to exercise every branch the callback takes.
 */
export const OAUTH_PROVIDER = 'google';

/** Label on the button that starts the flow. */
export const OAUTH_PROVIDER_NAME = 'Google';

/**
 * Configure the provider so the SPA offers it and the callback accepts it. The
 * credentials are never checked against anything — the stand-in behind
 * `OAUTH_ENDPOINT_BASE` takes whatever it's given.
 */
export async function seedOAuthProvider(): Promise<void> {
  await tengri([
    'oauth',
    'set',
    OAUTH_PROVIDER,
    JSON.stringify({
      client_id: 'e2e-client',
      client_secret: 'e2e-secret',
      visibility: 'public',
    }),
  ]);
}
