import { z } from 'zod';

/**
 * Full site-settings payload from `GET /admin/site` and `PATCH /admin/site`.
 * Includes the raw markdown columns the operator editor populates its textareas
 * from. Public callers should use `SiteIo` (`api/site.io.ts`) instead — this is
 * admin-only.
 */
export const AdminSiteIo = z.object({
  siteName: z.string(),
  canRegister: z.boolean(),
  tosMd: z.string().nullable(),
  privacyMd: z.string().nullable(),
});

export type AdminSite = z.infer<typeof AdminSiteIo>;

/**
 * Partial of {@link AdminSite}. JS doesn't distinguish absent from `undefined`,
 * so omit fields you don't want to touch; `null` on a markdown field clears the
 * column.
 */
export interface UpdateAdminSiteRequest {
  siteName?: string;
  canRegister?: boolean;
  tosMd?: string | null;
  privacyMd?: string | null;
}
