import { seedUser } from '../fixtures/user';
import { Header } from '../models/Header';
import { LoginModal } from '../models/LoginModal';
import { makeId } from '../support/ids';
import { expect, test } from '../support/test';

const PASSWORD = 'thermals4days';

test('a pilot signs in with their login, after missing twice', async ({
  page,
}) => {
  const account = await seedUser({
    login: `regular-${makeId()}`,
    name: 'Regular Pilot',
    email: `regular-${makeId()}@example.test`,
    password: PASSWORD,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();

  // A right login with a wrong password and a login nobody has are answered
  // with the same words, so neither says which half was wrong. Counted rather
  // than looked at: the second message is indistinguishable from the first,
  // including to this test.
  const refused = page.getByText('Wrong login or password');

  await modal.signIn({ identifier: account.login, password: 'notthisone1' });
  await expect(refused).toHaveCount(1);

  await modal.signIn({ identifier: `nobody-${makeId()}`, password: PASSWORD });
  await expect(refused).toHaveCount(2);

  await modal.signIn({ identifier: account.login, password: PASSWORD });
  await expect(header.signOut).toBeVisible();
  await expect(modal.root).toBeHidden();
});

test('a pilot signs in with their email address', async ({ page }) => {
  const account = await seedUser({
    login: `mailer-${makeId()}`,
    name: 'Mailer Pilot',
    email: `mailer-${makeId()}@example.test`,
    password: PASSWORD,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();

  // One field, matched against `login` *or* `email`.
  await modal.signIn({ identifier: account.email, password: PASSWORD });
  await expect(header.signOut).toBeVisible();
});

test('a banned pilot is told the account is disabled', async ({ page }) => {
  const account = await seedUser({
    login: `grounded-${makeId()}`,
    name: 'Grounded Pilot',
    email: `grounded-${makeId()}@example.test`,
    password: PASSWORD,
    // Nothing set, so not even `CAN_AUTHORIZE` — a disabled account.
    permissions: 0,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();

  // Named rather than folded into the generic refusal: the password checked
  // out, so saying so gives away nothing they couldn't already confirm, and
  // "wrong login or password" would send them round the reset loop forever.
  await modal.signIn({ identifier: account.login, password: PASSWORD });
  await expect(page.getByText('This account is disabled')).toBeVisible();
  await expect(header.signOut).toBeHidden();
});
