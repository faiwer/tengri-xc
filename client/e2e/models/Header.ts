import type { Locator, Page } from '@playwright/test';

/** The layout header's account controls. */
export class Header {
  /** Captioned "Sign in"; `Login` is its `aria-label`. Opens the account modal. */
  signIn: Locator;
  signOut: Locator;

  constructor(page: Page) {
    this.signIn = page.getByRole('button', { name: 'Login' });
    this.signOut = page.getByRole('button', { name: 'Sign out' });
  }
}
