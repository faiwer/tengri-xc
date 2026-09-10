import type { Locator, Page } from '@playwright/test';
import { findField } from '../support/forms';

/** The Profile tab of the settings page. */
export class ProfileSettings {
  name: Locator;
  email: Locator;

  private page: Page;

  constructor(page: Page) {
    this.page = page;
    const root = page.getByRole('main');
    this.name = findField(root, 'name');
    this.email = findField(root, 'email');
  }

  async open(): Promise<void> {
    await this.page.goto('/settings/profile');
  }
}
