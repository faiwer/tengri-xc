import { OAUTH_PROVIDER_NAME, seedOAuthLink } from '../fixtures/oauth';
import { seedUser } from '../fixtures/user';
import { AuthorizationSettings } from '../models/AuthorizationSettings';
import { Header } from '../models/Header';
import { LoginModal } from '../models/LoginModal';
import { ProfileSettings } from '../models/ProfileSettings';
import { findFieldError } from '../support/forms';
import { makeId } from '../support/ids';
import { findLink } from '../support/mail';
import { expect, test } from '../support/test';
import { getServerUrl } from '../support/urls';

const OLD_PASSWORD = 'oldthermals1';
const NEW_PASSWORD = 'freshthermals9';

test('a pilot changes their password', async ({ page }) => {
  const account = await seedUser({
    login: `keeper-${makeId()}`,
    name: 'Keeper Pilot',
    email: `keeper-${makeId()}@example.test`,
    password: OLD_PASSWORD,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signIn({ identifier: account.login, password: OLD_PASSWORD });
  await expect(header.signOut).toBeVisible();

  const authorization = new AuthorizationSettings(page);
  await authorization.open();

  // Holding the session isn't enough: the current password is what tells the
  // owner apart from whoever found the laptop unlocked.
  await authorization.changePassword({
    currentPassword: 'notthisone1',
    newPassword: NEW_PASSWORD,
  });
  await expect(
    findFieldError(authorization.root, 'currentPassword'),
  ).toHaveText('Incorrect password');

  await authorization.changePassword({
    currentPassword: OLD_PASSWORD,
    newPassword: NEW_PASSWORD,
  });
  await expect(page.getByText('Password saved')).toBeVisible();

  // The change signs every other session out, but hands this one a fresh
  // cookie rather than dropping it too.
  await expect(header.signOut).toBeVisible();
  await header.signOut.click();
  await expect(modal.root).toBeVisible();

  await modal.signIn({ identifier: account.login, password: OLD_PASSWORD });
  await expect(page.getByText('Wrong login or password')).toBeVisible();

  await modal.signIn({ identifier: account.login, password: NEW_PASSWORD });
  await expect(header.signOut).toBeVisible();
});

test('a pilot who signs in with a provider sets their first password', async ({
  page,
  provider,
}) => {
  const account = await seedUser({
    login: `passwordless-${makeId()}`,
    name: 'Passwordless Pilot',
    email: `passwordless-${makeId()}@example.test`,
  });
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Passwordless Pilot At Google',
    email: `passwordless-at-google-${makeId()}@example.test`,
    emailVerified: true,
  };
  await seedOAuthLink(account.login, identity);
  provider.signInAs(identity);

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signInWith(OAUTH_PROVIDER_NAME);
  await expect(header.signOut).toBeVisible();

  const authorization = new AuthorizationSettings(page);
  await authorization.open();

  // Nothing to prove: there's no current password to ask them for, so the field
  // isn't there at all.
  await expect(authorization.currentPassword).toHaveCount(0);
  await authorization.changePassword({ newPassword: NEW_PASSWORD });
  await expect(page.getByText('Password saved')).toBeVisible();

  // Which buys them a second way in, next to the provider they arrived with.
  await header.signOut.click();
  await expect(modal.root).toBeVisible();
  await modal.signIn({ identifier: account.login, password: NEW_PASSWORD });
  await expect(header.signOut).toBeVisible();
});

test('a pilot with no login picks one alongside their first password', async ({
  page,
  provider,
}) => {
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Unnamed Newcomer',
    email: `unnamed-${makeId()}@example.test`,
    emailVerified: true,
  };
  provider.signInAs(identity);

  const header = new Header(page);
  const modal = new LoginModal(page);

  // Registering with a provider is how an account ends up with neither a login
  // nor a password: the provider vouches for an address, and that's all.
  await page.goto('/');
  await header.signIn.click();
  await modal.signInWith(OAUTH_PROVIDER_NAME);
  await expect(header.signOut).toBeVisible();

  const authorization = new AuthorizationSettings(page);
  await authorization.open();

  // Editable this once, and the form won't save a password without it — a
  // login they never chose isn't one they could sign in with.
  await expect(authorization.login).toBeEnabled();
  await expect(authorization.login).toHaveValue('');
  await authorization.changePassword({ newPassword: NEW_PASSWORD });
  await expect(findFieldError(authorization.root, 'login')).toHaveText(
    'Choose a login',
  );

  const login = `chosen-${makeId()}`;
  await authorization.changePassword({ login, newPassword: NEW_PASSWORD });
  await expect(page.getByText('Password saved')).toBeVisible();

  // Both halves of that save landed, so the pair works on its own.
  await header.signOut.click();
  await expect(modal.root).toBeVisible();
  await modal.signIn({ identifier: login, password: NEW_PASSWORD });
  await expect(header.signOut).toBeVisible();
});

test('a pilot cannot change the login they already have', async ({ page }) => {
  const account = await seedUser({
    login: `fixed-${makeId()}`,
    name: 'Fixed Pilot',
    email: `fixed-${makeId()}@example.test`,
    password: OLD_PASSWORD,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signIn({ identifier: account.login, password: OLD_PASSWORD });
  await expect(header.signOut).toBeVisible();

  const authorization = new AuthorizationSettings(page);
  await authorization.open();
  await expect(authorization.login).toHaveValue(account.login);
  await expect(authorization.login).toBeDisabled();

  // The form sends no login at all, so the set-once rule behind it is only
  // reachable by asking the endpoint directly — with the cookie this browser
  // is already carrying.
  const forced = await page.request.post(
    `${getServerUrl()}/users/me/password`,
    {
      data: {
        login: `hijacked-${makeId()}`,
        current_password: OLD_PASSWORD,
        new_password: NEW_PASSWORD,
      },
    },
  );
  expect(forced.status()).toBe(200);

  // Dropped rather than refused — and the password change it came wrapped in
  // still went through.
  await page.reload();
  await expect(authorization.login).toHaveValue(account.login);

  await header.signOut.click();
  await expect(modal.root).toBeVisible();
  await modal.signIn({ identifier: account.login, password: NEW_PASSWORD });
  await expect(header.signOut).toBeVisible();
});

test('a pilot moves their account to another email address', async ({
  page,
  mailbox,
}) => {
  const other = await seedUser({
    login: `holder-${makeId()}`,
    name: 'Holder Pilot',
    email: `holder-${makeId()}@example.test`,
  });
  const account = await seedUser({
    login: `mover-${makeId()}`,
    name: 'Mover Pilot',
    email: `mover-${makeId()}@example.test`,
    password: OLD_PASSWORD,
  });
  const newEmail = `moved-${makeId()}@example.test`;

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signIn({ identifier: account.login, password: OLD_PASSWORD });
  await expect(header.signOut).toBeVisible();

  // Saving validates the whole profile and a seeded account carries no sex, so
  // that one gets filled in out of band — the save under test is the address.
  const seeded = await page.request.patch(`${getServerUrl()}/users/me`, {
    data: { profile: { sex: 'female' } },
  });
  expect(seeded.status()).toBe(200);

  const profile = new ProfileSettings(page);
  await profile.open();

  // Someone else has already proven this address, so it isn't free to claim.
  await profile.email.fill(other.email);
  await profile.save.click();
  await expect(findFieldError(profile.root, 'email')).toHaveText(
    'Already taken',
  );
  expect(mailbox.countFor(other.email)).toBe(0);

  await profile.email.fill(newEmail);
  await profile.save.click();
  await expect(page.getByText('Confirm your new email')).toBeVisible();

  // The new address is only pending until the link is opened, so mail keeps
  // reaching them where it did before — including this confirmation.
  await page.reload();
  await expect(profile.email).toHaveValue(account.email);

  const mail = await mailbox.take(newEmail);
  await page.goto(findLink(mail.body, '/users/confirm-email'));
  await expect(page.getByText('Email address updated')).toBeVisible();

  await profile.open();
  await expect(profile.email).toHaveValue(newEmail);

  await header.signOut.click();
  await expect(modal.root).toBeVisible();

  // The old address belongs to nobody now, which the sign-in form can't tell
  // apart from a wrong password.
  await modal.signIn({ identifier: account.email, password: OLD_PASSWORD });
  await expect(page.getByText('Wrong login or password')).toBeVisible();

  await modal.signIn({ identifier: newEmail, password: OLD_PASSWORD });
  await expect(header.signOut).toBeVisible();
});

test('a confirmation link that carries nothing usable is turned away', async ({
  page,
}) => {
  const header = new Header(page);

  await page.goto(`${getServerUrl()}/users/confirm-email?token=not-a-token`);

  // Every outcome of that endpoint lands on the SPA with a code in the query,
  // so a mangled link reads as one sentence here rather than as an API error.
  await expect(page.getByText("That link isn't valid")).toBeVisible();
  await expect(header.signOut).toBeHidden();
});
