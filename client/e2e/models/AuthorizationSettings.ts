import type { Locator, Page } from '@playwright/test';
import { findField } from '../support/forms';

/**
 * What to put in the password form. `login` and `currentPassword` are left out
 * whenever the form doesn't render them: the first is offered only to an
 * account that has no login yet, the second only to one that has a password.
 */
export interface PasswordChange {
  newPassword: string;
  login?: string;
  currentPassword?: string;
}

/** The Authorization tab of the settings page: password, plus social links. */
export class AuthorizationSettings {
  /** The page body, for asserting on validation messages and copy. */
  root: Locator;
  login: Locator;
  currentPassword: Locator;
  newPassword: Locator;
  repeatPassword: Locator;
  save: Locator;

  private page: Page;

  constructor(page: Page) {
    this.page = page;
    this.root = page.getByRole('main');
    this.login = findField(this.root, 'login');
    this.currentPassword = findField(this.root, 'currentPassword');
    this.newPassword = findField(this.root, 'newPassword');
    this.repeatPassword = findField(this.root, 'repeatPassword');
    this.save = this.root.getByRole('button', { name: 'Save' });
  }

  async open(): Promise<void> {
    await this.page.goto('/settings/authorization');
  }

  /** Fills the password form and saves. The repeat field gets the same value. */
  async changePassword(change: PasswordChange): Promise<void> {
    if (change.login != null) {
      await this.login.fill(change.login);
    }
    if (change.currentPassword != null) {
      await this.currentPassword.fill(change.currentPassword);
    }
    await this.newPassword.fill(change.newPassword);
    await this.repeatPassword.fill(change.newPassword);
    await this.save.click();
  }

  /** One of the provider buttons under "Add a new account". */
  linkWith(provider: string): Promise<void> {
    return this.root.getByRole('button', { name: `Link ${provider}` }).click();
  }

  /**
   * The connected-accounts entry for `provider`, carrying the name and address
   * the link snapshot holds. Found by its unlink button, the only thing in the
   * row with a name of its own.
   */
  connection(provider: string): Locator {
    return this.root.getByRole('listitem').filter({
      has: this.page.getByRole('button', { name: `Unlink ${provider}` }),
    });
  }

  /** Removes a connected account, through the confirmation it asks for. */
  async unlink(provider: string): Promise<void> {
    await this.connection(provider)
      .getByRole('button', { name: `Unlink ${provider}` })
      .click();
    // The confirmation is a popover portalled to the body, so it sits outside
    // `main`. Exact, or the row's own button matches it too.
    await this.page
      .getByRole('button', { name: 'Unlink', exact: true })
      .click();
  }
}
