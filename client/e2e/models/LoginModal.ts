import type { Locator, Page } from '@playwright/test';
import { findField } from '../support/forms';

export interface NewAccount {
  login: string;
  name: string;
  email: string;
  password: string;
}

export interface Credentials {
  /** Sign-in accepts either the login or the email address. */
  identifier: string;
  password: string;
}

/**
 * The account modal — one dialog whose title and body swap between signing
 * in, registering, and resetting a password.
 */
export class LoginModal {
  /** The dialog itself, for asserting on titles and post-submit copy. */
  root: Locator;

  constructor(page: Page) {
    this.root = page.getByRole('dialog');
  }

  /** Footer link to registration, rendered only while `can_register` is on. */
  openRegister(): Promise<void> {
    return this.root.getByText('Register', { exact: true }).click();
  }

  async fillNewAccount(account: NewAccount): Promise<void> {
    await findField(this.root, 'login').fill(account.login);
    await findField(this.root, 'name').fill(account.name);
    await findField(this.root, 'email').fill(account.email);
    await findField(this.root, 'password').fill(account.password);
    await findField(this.root, 'repeatPassword').fill(account.password);
  }

  submitRegistration(): Promise<void> {
    return this.root.getByRole('button', { name: 'Register' }).click();
  }

  /** Fills and submits the sign-in form the modal opens on. */
  async signIn(credentials: Credentials): Promise<void> {
    await findField(this.root, 'identifier').fill(credentials.identifier);
    await findField(this.root, 'password').fill(credentials.password);
    await this.root.getByRole('button', { name: 'Sign in' }).click();
  }

  /** The `×` in the corner, which every form in the dialog shares. */
  close(): Promise<void> {
    return this.root.getByRole('button', { name: 'Close' }).click();
  }
}
