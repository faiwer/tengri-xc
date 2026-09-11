import type { Locator, Page } from '@playwright/test';

/** The Authorization tab of the settings page: password, plus social links. */
export class AuthorizationSettings {
  private page: Page;
  private root: Locator;

  constructor(page: Page) {
    this.page = page;
    this.root = page.getByRole('main');
  }

  async open(): Promise<void> {
    await this.page.goto('/settings/authorization');
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
