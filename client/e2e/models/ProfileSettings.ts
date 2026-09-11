import type { Locator, Page } from '@playwright/test';
import { findField } from '../support/forms';

/** The Profile tab of the settings page. */
export class ProfileSettings {
  /** The page body, for asserting on validation messages and copy. */
  root: Locator;
  name: Locator;
  email: Locator;
  /** Rendered only while the form holds unsaved changes. */
  save: Locator;

  private page: Page;

  constructor(page: Page) {
    this.page = page;
    this.root = page.getByRole('main');
    this.name = findField(this.root, 'name');
    this.email = findField(this.root, 'email');
    this.save = this.root.getByRole('button', { name: 'Save' });
  }

  async open(): Promise<void> {
    await this.page.goto('/settings/profile');
  }
}
