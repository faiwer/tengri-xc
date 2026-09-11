import { SMTP_PORT } from '../support/fakeSmtp';
import { tengri } from '../support/tengri';

/** From-address on every message the suite receives. */
export const MAIL_FROM = 'noreply@tengri.test';

/** Site name, which mail subjects are built from. */
export const SITE_NAME = 'Tengri XC';

/** Site settings a single test is allowed to change; see the `site` fixture. */
export interface SiteControls {
  setCanRegister(enabled: boolean): Promise<void>;
}

export async function setCanRegister(enabled: boolean): Promise<void> {
  await tengri(['site', 'set', JSON.stringify({ can_register: enabled })]);
}

export async function seedSiteSettings(): Promise<void> {
  await tengri([
    'site',
    'set',
    JSON.stringify({
      site_name: SITE_NAME,
      can_register: true,
      smtp_host: '127.0.0.1',
      smtp_port: SMTP_PORT,
      smtp_tls: 'none',
      smtp_username: null,
      smtp_password: null,
      from_address: MAIL_FROM,
      title_template: '%title%',
      body_template: '%body%',
    }),
  ]);
}
