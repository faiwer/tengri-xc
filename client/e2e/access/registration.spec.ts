import { MAIL_FROM, SITE_NAME } from '../fixtures/site';
import { seedUser } from '../fixtures/user';
import { Header } from '../models/Header';
import { LoginModal, type NewAccount } from '../models/LoginModal';
import { ProfileSettings } from '../models/ProfileSettings';
import { findFieldError } from '../support/forms';
import { makeId } from '../support/ids';
import { findLink } from '../support/mail';
import { expect, test } from '../support/test';

test('a visitor registers, confirms by mail, and lands signed in', async ({
  page,
  mailbox,
}) => {
  const account: NewAccount = {
    login: `pilot-${makeId()}`,
    name: 'New Pilot',
    email: `pilot-${makeId()}@example.test`,
    password: 'thermals4days',
  };

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();

  await modal.openRegister();
  await expect(modal.root).toContainText('New account');

  await modal.fillNewAccount(account);
  await modal.submitRegistration();

  await expect(modal.root).toContainText("We've sent a confirmation link to");
  await expect(modal.root).toContainText(account.email);

  const mail = await mailbox.take(account.email);
  expect(mail.from).toBe(MAIL_FROM);
  expect(mail.subject).toBe(`Confirm your ${SITE_NAME} account`);
  expect(mail.contentType).toContain('text/html');
  expect(mail.body).toContain('Confirm the email address');

  // The row exists and the password is right, but the address is still unproven
  // — the server answers 403 `email_unconfirmed` instead of a session.
  await modal.close();
  await header.signIn.click();
  await modal.signIn({
    identifier: account.login,
    password: account.password,
  });
  await expect(
    page.getByText('Please check your email for a confirmation link'),
  ).toBeVisible();
  await expect(header.signOut).toBeHidden();

  const link = findLink(mail.body, '/users/confirm-email');
  expect(link).toContain(`/users/confirm-email?token=`);

  // Registration hands back no session; the link is what signs you in.
  await page.goto(link);
  await expect(page.getByText('Email confirmed')).toBeVisible();
  await expect(header.signOut).toBeVisible();

  const profile = new ProfileSettings(page);
  await profile.open();
  await expect(profile.name).toHaveValue(account.name);
  await expect(profile.email).toHaveValue(account.email);
});

test('a visitor registers an account with an existing login, email, name', async ({
  page,
  mailbox,
}) => {
  const taken = await seedUser({
    // A display name can't carry a digit, so this one leans on the fresh
    // database each run gives it rather than on a random suffix.
    name: 'Taken Pilot',
    login: `taken-${makeId()}`,
    email: `taken-${makeId()}@example.test`,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.openRegister();

  // An empty form marks up every field at once rather than stopping at the
  // first, and nothing leaves the browser.
  await modal.submitRegistration();
  for (const field of REGISTER_FIELDS) {
    await expect(findFieldError(modal.root, field)).toHaveText('Required');
  }

  // Same again from the server: all three collisions come back in one 422,
  // each on the field that caused it.
  await modal.fillNewAccount({ ...taken, password: 'thermals4days' });
  await modal.submitRegistration();
  await expect(findFieldError(modal.root, 'login')).toHaveText('Already taken');
  await expect(findFieldError(modal.root, 'name')).toHaveText('Already taken');
  await expect(findFieldError(modal.root, 'email')).toHaveText('Already taken');

  // Still the form, not the "check your inbox" panel that a signup reaches.
  await expect(modal.root).toContainText('New account');
  expect(mailbox.countFor(taken.email)).toBe(0);
});

const REGISTER_FIELDS = [
  'login',
  'name',
  'email',
  'password',
  'repeatPassword',
];
