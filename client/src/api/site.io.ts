import { z } from 'zod';

/**
 * Slim site-settings payload from `GET /site`. Loaded once at app boot and held
 * in the `useSite()` context. `hasTos` / `hasPrivacy` drive footer-link
 * visibility without paying for the markdown bytes on every page load — the
 * long-form content rides separate on-demand endpoints (`/site/tos`,
 * `/site/privacy`).
 */
export const SiteIo = z.object({
  siteName: z.string(),
  canRegister: z.boolean(),
  hasTos: z.boolean(),
  hasPrivacy: z.boolean(),
});

export type Site = z.infer<typeof SiteIo>;

/** Which long-form document a `/site/:kind` fetch is asking about. */
export type DocKind = 'tos' | 'privacy';

/** Wrapper for `GET /site/tos` and `GET /site/privacy`. */
export const SiteDocumentIo = z.object({
  md: z.string(),
});
