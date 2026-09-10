import { test as base } from '@playwright/test';
import { type FakeOAuth, startFakeOAuth } from './fakeOAuth';
import { type FakeSmtp, type Mailbox, startFakeSmtp } from './fakeSmtp';

export { expect } from '@playwright/test';

export const test = base.extend<
  { mailbox: Mailbox; provider: FakeOAuth },
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
});
