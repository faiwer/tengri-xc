import { test as base } from '@playwright/test';
import {
  seedSiteSettings,
  setCanRegister,
  type SiteControls,
} from '../fixtures/site';
import { type FakeOAuth, startFakeOAuth } from './fakeOAuth';
import { type FakeSmtp, type Mailbox, startFakeSmtp } from './fakeSmtp';

export { expect } from '@playwright/test';

export const test = base.extend<
  { mailbox: Mailbox; provider: FakeOAuth; site: SiteControls },
  { smtp: FakeSmtp; oauth: FakeOAuth }
>({
  smtp: [
    // oxlint-disable-next-line no-empty-pattern -- Playwright reads a fixture's dependencies out of this destructuring, so it can't be a plain parameter.
    async ({}, use) => {
      const smtp = await startFakeSmtp();
      await use(smtp);
      await smtp.close();
    },
    { scope: 'worker' },
  ],

  oauth: [
    // oxlint-disable-next-line no-empty-pattern -- Same as `smtp` above.
    async ({}, use) => {
      const oauth = await startFakeOAuth();
      await use(oauth);
      await oauth.close();
    },
    { scope: 'worker' },
  ],

  mailbox: async ({ smtp }, use) => {
    smtp.mailbox.clear();
    await use(smtp.mailbox);
  },

  provider: async ({ oauth }, use) => {
    oauth.reset();
    await use(oauth);
  },

  // oxlint-disable-next-line no-empty-pattern -- See `smtp` above.
  site: async ({}, use) => {
    let changed = false;
    await use({
      setCanRegister: async (enabled) => {
        changed = true;
        await setCanRegister(enabled);
      },
    });

    // Restored to the seeded baseline rather than to whatever it was: these
    // settings are shared by the whole run, and every spec that registers an
    // account reads them.
    if (changed) {
      await seedSiteSettings();
    }
  },
});
