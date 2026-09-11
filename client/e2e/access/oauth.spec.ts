import { OAUTH_PROVIDER_NAME, seedOAuthLink } from '../fixtures/oauth';
import { seedUser } from '../fixtures/user';
import { AuthorizationSettings } from '../models/AuthorizationSettings';
import { Header } from '../models/Header';
import { LoginModal } from '../models/LoginModal';
import { ProfileSettings } from '../models/ProfileSettings';
import { makeId } from '../support/ids';
import { expect, test } from '../support/test';

test('a visitor registers with a provider account', async ({
  page,
  provider,
}) => {
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Soaring Newcomer',
    email: `soaring-${makeId()}@example.test`,
    emailVerified: true,
  };
  provider.signInAs(identity);

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signInWith(OAUTH_PROVIDER_NAME);

  // Nobody has this subject or this address, so the callback creates an
  // account and signs it in — no confirmation step, since the provider is the
  // one vouching for the address.
  await expect(page.getByText('Welcome')).toBeVisible();
  await expect(header.signOut).toBeVisible();

  // A fresh account lands on its profile to fill in the rest.
  await expect(page).toHaveURL(/\/settings\/profile$/);
  const profile = new ProfileSettings(page);
  await expect(profile.name).toHaveValue(identity.name);
  await expect(profile.email).toHaveValue(identity.email);
});

test('a user signs in with the provider account they registered with', async ({
  page,
  provider,
}) => {
  const account = await seedUser({
    login: `returning-${makeId()}`,
    name: 'Returning Pilot',
    email: `returning-${makeId()}@example.test`,
  });
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Returning Pilot At Google',
    // Deliberately not the account's own address: the link is the only thing
    // tying this identity to it, so a pass can't come from the email match.
    email: `returning-at-google-${makeId()}@example.test`,
    emailVerified: true,
  };
  await seedOAuthLink(account.login, identity);
  provider.signInAs(identity);

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signInWith(OAUTH_PROVIDER_NAME);

  // "Signed in" rather than "Welcome", and back where they started rather than
  // on the profile page a fresh account lands on.
  await expect(page.getByText('Signed in')).toBeVisible();
  await expect(header.signOut).toBeVisible();
  await expect(page).toHaveURL(/\/$/);

  // Their own details, not the snapshot the provider just sent.
  const profile = new ProfileSettings(page);
  await profile.open();
  await expect(profile.name).toHaveValue(account.name);
  await expect(profile.email).toHaveValue(account.email);
});

test('a pilot signing in with a provider joins the account holding that address', async ({
  page,
  provider,
}) => {
  const account = await seedUser({
    login: `matched-${makeId()}`,
    name: 'Matched Pilot',
    email: `matched-${makeId()}@example.test`,
  });

  provider.signInAs({
    // Nothing links this subject to the account yet; the address is all the
    // two have in common.
    sub: `subject-${makeId()}`,
    name: 'Whatever Google Calls Them',
    email: account.email,
    emailVerified: true,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signInWith(OAUTH_PROVIDER_NAME);

  await expect(page.getByText('Signed in')).toBeVisible();
  await expect(header.signOut).toBeVisible();

  // The account they landed in is the seeded one, down to the name the
  // provider would have overwritten had this registered instead.
  const profile = new ProfileSettings(page);
  await profile.open();
  await expect(profile.name).toHaveValue(account.name);
  await expect(profile.email).toHaveValue(account.email);

  // And the identity is attached, so the next sign-in matches on the subject
  // rather than going through the address again.
  const authorization = new AuthorizationSettings(page);
  await authorization.open();
  await expect(authorization.connection(OAUTH_PROVIDER_NAME)).toContainText(
    account.email,
  );
});

test('a pilot adds a provider account from the settings page', async ({
  page,
  provider,
}) => {
  const password = 'thermals4days';
  const account = await seedUser({
    login: `settled-${makeId()}`,
    name: 'Settled Pilot',
    email: `settled-${makeId()}@example.test`,
    password,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signIn({ identifier: account.login, password });
  await expect(header.signOut).toBeVisible();

  // A different address from the account's own, so the link can't be mistaken
  // for the email match the login flow does.
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Settled Pilot At Google',
    email: `settled-at-google-${makeId()}@example.test`,
    emailVerified: true,
  };
  provider.signInAs(identity);

  const authorization = new AuthorizationSettings(page);
  await authorization.open();
  await authorization.linkWith(OAUTH_PROVIDER_NAME);

  // The link intent mints no session — the one from the password sign-in is
  // still what carries them.
  await expect(page.getByText('Account linked')).toBeVisible();
  await expect(header.signOut).toBeVisible();

  const connection = authorization.connection(OAUTH_PROVIDER_NAME);
  await expect(connection).toContainText(identity.name);
  await expect(connection).toContainText(identity.email);

  // The provider's address is the link's, not the account's.
  const profile = new ProfileSettings(page);
  await profile.open();
  await expect(profile.email).toHaveValue(account.email);
});

test('a pilot removes a provider account on the settings page', async ({
  page,
}) => {
  const password = 'thermals4days';
  const account = await seedUser({
    login: `parting-${makeId()}`,
    name: 'Parting Pilot',
    email: `parting-${makeId()}@example.test`,
    password,
  });
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Parting Pilot At Google',
    email: `parting-at-google-${makeId()}@example.test`,
  };
  await seedOAuthLink(account.login, identity);

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signIn({ identifier: account.login, password });
  await expect(header.signOut).toBeVisible();

  const authorization = new AuthorizationSettings(page);
  await authorization.open();
  const connection = authorization.connection(OAUTH_PROVIDER_NAME);
  await expect(connection).toContainText(identity.email);

  // The password is their other way in, so nothing stops the removal.
  await authorization.unlink(OAUTH_PROVIDER_NAME);
  await expect(connection).toHaveCount(0);

  // Gone from the row the server hands back too, not just from the list this
  // page was holding.
  await page.reload();
  await expect(connection).toHaveCount(0);
  await expect(page.getByText(identity.email)).toBeHidden();
});

test('a pilot cannot link a provider account that belongs to someone else', async ({
  page,
  provider,
}) => {
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Owner Pilot At Google',
    email: `owner-at-google-${makeId()}@example.test`,
    emailVerified: true,
  };
  const owner = await seedUser({
    login: `owner-${makeId()}`,
    name: 'Owner Pilot',
    email: `owner-${makeId()}@example.test`,
  });
  await seedOAuthLink(owner.login, identity);

  const password = 'thermals4days';
  const intruder = await seedUser({
    login: `intruder-${makeId()}`,
    name: 'Intruder Pilot',
    email: `intruder-${makeId()}@example.test`,
    password,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signIn({ identifier: intruder.login, password });
  await expect(header.signOut).toBeVisible();

  // The provider consents as the owner's account — proving a subject to us is
  // not the same as it being free to claim.
  provider.signInAs(identity);

  const authorization = new AuthorizationSettings(page);
  await authorization.open();
  await authorization.linkWith(OAUTH_PROVIDER_NAME);

  await expect(
    page.getByText('That account is already linked to a different Tengri user'),
  ).toBeVisible();

  // Refused outright rather than moved: the link is still the owner's alone.
  await expect(authorization.connection(OAUTH_PROVIDER_NAME)).toHaveCount(0);
});

test('a visitor is told when the provider flow falls over', async ({
  page,
  provider,
}) => {
  provider.signInAs({
    sub: `subject-${makeId()}`,
    name: 'Unlucky Pilot',
    email: `unlucky-${makeId()}@example.test`,
    emailVerified: true,
  });
  provider.breakTokenExchange();

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signInWith(OAUTH_PROVIDER_NAME);

  // Consent went through, so the browser is back on our side by the time the
  // exchange fails — which is why this surfaces as a toast on our page rather
  // than as the provider's error screen.
  await expect(
    page.getByText('Something went wrong during authorization'),
  ).toBeVisible();
  await expect(header.signIn).toBeVisible();
  await expect(header.signOut).toBeHidden();
});

test('a banned pilot cannot sign in with their provider account', async ({
  page,
  provider,
}) => {
  const account = await seedUser({
    login: `grounded-${makeId()}`,
    name: 'Grounded Flyer',
    email: `grounded-${makeId()}@example.test`,
    // Nothing set, so not even `CAN_AUTHORIZE` — a disabled account.
    permissions: 0,
  });
  const identity = {
    sub: `subject-${makeId()}`,
    name: 'Grounded Flyer At Google',
    email: `grounded-at-google-${makeId()}@example.test`,
    emailVerified: true,
  };
  await seedOAuthLink(account.login, identity);
  provider.signInAs(identity);

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.signInWith(OAUTH_PROVIDER_NAME);

  // The link resolves to the account fine; it's the session the server won't
  // mint. Named, like the password path's refusal, because there's nothing to
  // hide from someone who just proved they own the identity.
  await expect(
    page.getByText('This account is banned and cannot sign in'),
  ).toBeVisible();
  await expect(header.signOut).toBeHidden();
});

test('a visitor cannot register with a provider while registration is off', async ({
  page,
  provider,
  site,
}) => {
  await site.setCanRegister(false);

  provider.signInAs({
    sub: `subject-${makeId()}`,
    name: 'Late Arrival',
    email: `late-${makeId()}@example.test`,
    emailVerified: true,
  });

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();

  // The switch takes the register link with it, but not the provider buttons:
  // they're how the accounts that already exist sign in.
  await expect(modal.register).toHaveCount(0);
  await modal.signInWith(OAUTH_PROVIDER_NAME);

  // Nothing to match this identity to, and nothing may be created for it — so
  // the refusal comes after the provider has already consented.
  await expect(
    page.getByText('Registration is currently disabled'),
  ).toBeVisible();
  await expect(header.signOut).toBeHidden();
});
