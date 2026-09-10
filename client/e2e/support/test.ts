import { test as base } from '@playwright/test';
import { type FakeSmtp, type Mailbox, startFakeSmtp } from './fakeSmtp';

export { expect } from '@playwright/test';

export const test = base.extend<{ mailbox: Mailbox }, { smtp: FakeSmtp }>({
  smtp: [
    // oxlint-disable-next-line no-empty-pattern -- Playwright reads a fixture's dependencies out of this destructuring, so it can't be a plain parameter.
    async ({}, use) => {
      const smtp = await startFakeSmtp();
      await use(smtp);
      await smtp.close();
    },
    { scope: 'worker' },
  ],

  mailbox: async ({ smtp }, use) => {
    smtp.mailbox.clear();
    await use(smtp.mailbox);
  },
});
