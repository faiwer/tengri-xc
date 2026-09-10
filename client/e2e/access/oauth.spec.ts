import { OAUTH_PROVIDER_NAME } from '../fixtures/oauth';
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
