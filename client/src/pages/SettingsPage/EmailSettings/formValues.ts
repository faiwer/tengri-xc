import type {
  AdminSite,
  SmtpTls,
  UpdateAdminSiteRequest,
} from '../../../api/admin/site.io';

export type EmailSettingsFormValues = {
  smtpHost: string;
  smtpPort: number | null;
  smtpTls: SmtpTls | null;
  smtpUsername: string;
  smtpPassword: string;
  fromAddress: string;
  titleTemplate: string;
  bodyTemplate: string;
};

export function toFormValues(site: AdminSite): EmailSettingsFormValues {
  return {
    // Form fields are non-null strings; `null` in the DB renders as an empty
    // input, and submitting empty round-trips back to NULL via the server's
    // "empty string = clear" rule.
    smtpHost: site.smtpHost ?? '',
    smtpPort: site.smtpPort,
    smtpTls: site.smtpTls,
    smtpUsername: site.smtpUsername ?? '',
    // The stored secret never enters form state. Blank means "leave unchanged"
    // server-side, so an untouched save can't wipe it.
    smtpPassword: '',
    fromAddress: site.fromAddress ?? '',
    titleTemplate: site.titleTemplate,
    bodyTemplate: site.bodyTemplate,
  };
}

export function fromFormValues(
  values: EmailSettingsFormValues,
): UpdateAdminSiteRequest {
  return {
    smtpHost: values.smtpHost,
    smtpPort: values.smtpPort ?? null,
    smtpTls: values.smtpTls ?? null,
    smtpUsername: values.smtpUsername,
    smtpPassword: values.smtpPassword,
    fromAddress: values.fromAddress,
    titleTemplate: values.titleTemplate,
    bodyTemplate: values.bodyTemplate,
  };
}
