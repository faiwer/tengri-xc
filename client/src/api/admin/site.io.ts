import { z } from 'zod';

/**
 * How the SMTP client speaks TLS. `implicit` is SMTPS (typically port 465),
 * `starttls` upgrades a plain connection (typically 587), `none` is plaintext.
 * Declared above {@link AdminSiteIo} because the schema references it.
 */
export const SmtpTlsIo = z.enum(['implicit', 'starttls', 'none']);

export type SmtpTls = z.infer<typeof SmtpTlsIo>;

/**
 * Full site-settings payload from `GET /admin/site` and `PATCH /admin/site`.
 * Includes the raw markdown columns the operator editor populates its textareas
 * from. Public callers should use `SiteIo` (`api/site.io.ts`) instead — this is
 * admin-only.
 */
export const AdminSiteIo = z.object({
  siteName: z.string(),
  siteDescription: z.string().nullable(),
  canRegister: z.boolean(),
  tosMd: z.string().nullable(),
  privacyMd: z.string().nullable(),
  smtpHost: z.string().nullable(),
  smtpPort: z.number().nullable(),
  smtpTls: SmtpTlsIo.nullable(),
  smtpUsername: z.string().nullable(),
  smtpPassword: z.string().nullable(),
  fromAddress: z.string().nullable(),
  titleTemplate: z.string(),
  bodyTemplate: z.string(),
});

export type AdminSite = z.infer<typeof AdminSiteIo>;

/**
 * Partial of {@link AdminSite}. JS doesn't distinguish absent from `undefined`,
 * so omit fields you don't want to touch; `null` on a markdown field clears the
 * column.
 *
 * `smtpPassword` is the exception to "empty clears": the server reads a blank
 * string as "leave the stored password alone", so clearing it takes an explicit
 * `null`.
 */
/**
 * Body of `POST /admin/site/test-email`. Unlike {@link UpdateAdminSiteRequest}
 * this is not a partial: the editor submits its whole current state so an
 * unsaved connection can be tried before it's committed. `smtpPort` and
 * `smtpTls` are nullable because the form's controls are clearable.
 */
export interface SendTestEmailRequest {
  /** Where to deliver the probe. */
  to: string;
  smtpHost: string;
  smtpPort: number | null;
  smtpTls: SmtpTls | null;
  smtpUsername: string;
  /** Blank keeps the stored secret, same as on a save. */
  smtpPassword: string;
  fromAddress: string;
  titleTemplate: string;
  bodyTemplate: string;
}

export interface UpdateAdminSiteRequest {
  siteName?: string;
  siteDescription?: string | null;
  canRegister?: boolean;
  tosMd?: string | null;
  privacyMd?: string | null;
  smtpHost?: string | null;
  smtpPort?: number | null;
  smtpTls?: SmtpTls | null;
  smtpUsername?: string | null;
  smtpPassword?: string | null;
  fromAddress?: string | null;
  titleTemplate?: string;
  bodyTemplate?: string;
}
