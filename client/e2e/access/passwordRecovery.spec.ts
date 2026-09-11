import { SITE_NAME } from '../fixtures/site';
import { seedUser } from '../fixtures/user';
import { Header } from '../models/Header';
import { LoginModal } from '../models/LoginModal';
import { ResetPasswordPage } from '../models/ResetPasswordPage';
import { makeId } from '../support/ids';
import { findLink } from '../support/mail';
import { expect, test } from '../support/test';

test('a visitor recovers a forgotten password', async ({ page, mailbox }) => {
  const account = await seedUser({
    login: `forgetful-${makeId()}`,
    name: 'Forgetful Pilot',
    email: `forgetful-${makeId()}@example.test`,
    password: 'oldthermals1',
  });
  const newPassword = 'freshthermals9';

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.openResetPassword();
  await modal.requestResetPassword(account.email);

  // Worded the same whether or not the address belongs to an account.
  await expect(modal.root).toContainText(
    'a link to choose a new password is on its way',
  );

  const mail = await mailbox.take(account.email);
  expect(mail.subject).toBe(`Reset your ${SITE_NAME} password`);

  // Unlike the confirmation mail, this one points at the SPA: the click has no
  // side effect, it just opens the form.
  const link = findLink(mail.body, '/reset-password');
  await page.goto(link);

  const reset = new ResetPasswordPage(page);
  await reset.setPassword(newPassword);

  // Setting the password signs them in and lands them where they can change it
  // again.
  await expect(page).toHaveURL(/\/settings\/authorization$/);
  await expect(header.signOut).toBeVisible();

  // Signing out routes through `/login`, which bounces home with the modal
  // already open.
  await header.signOut.click();
  await expect(modal.root).toBeVisible();

  await modal.signIn({ identifier: account.login, password: newPassword });
  await expect(header.signOut).toBeVisible();

  // The link named one send, and that send is spent. Nothing on the page can
  // tell until it's submitted, so the form comes up again and then replaces
  // itself with the refusal.
  await page.goto(link);
  await reset.setPassword('anotherthermal7');
  await expect(page.getByText("That link isn't valid any more")).toBeVisible();
  await expect(reset.submit).toHaveCount(0);
});

test('a visitor asks to recover an address nobody registered', async ({
  page,
  mailbox,
}) => {
  const stranger = `nobody-${makeId()}@example.test`;

  const header = new Header(page);
  const modal = new LoginModal(page);

  await page.goto('/');
  await header.signIn.click();
  await modal.openResetPassword();
  await modal.requestResetPassword(stranger);

  // Same panel, same address read back, as for an address that does have an
  // account: a typo looks exactly like a link on its way, and the form can't
  // be used to find out who is registered here.
  await expect(modal.root).toContainText(
    'a link to choose a new password is on its way',
  );
  await expect(modal.root).toContainText(stranger);

  // The mail would have gone out before the server answered, so nothing can
  // still be in flight by the time the panel is up.
  expect(mailbox.countFor(stranger)).toBe(0);
});
