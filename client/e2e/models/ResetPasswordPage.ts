import type { Locator, Page } from '@playwright/test';
import { findField } from '../support/forms';

/** `/reset-password?token=…`, where the link in a reset mail lands. */
export class ResetPasswordPage {
  password: Locator;
  repeatPassword: Locator;
  submit: Locator;

  constructor(page: Page) {
    const root = page.getByRole('main');
    this.password = findField(root, 'password');
    this.repeatPassword = findField(root, 'repeatPassword');
    this.submit = root.getByRole('button', {
      name: 'Set the password and sign in',
    });
  }

  async setPassword(password: string): Promise<void> {
    await this.password.fill(password);
    await this.repeatPassword.fill(password);
    await this.submit.click();
  }
}
