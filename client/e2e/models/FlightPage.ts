import type { Download, Locator, Page } from '@playwright/test';

/** A flight's own page: the metadata sidebar and the owner's actions menu. */
export class FlightPage {
  /** Pilot, date, glider and the route figures. */
  meta: Locator;

  private page: Page;

  constructor(page: Page) {
    this.page = page;
    this.meta = page.getByRole('region', { name: 'Flight metadata' });
  }

  /** Opens the edit dialog. Only the owner and admins have the menu at all. */
  async edit(): Promise<void> {
    await this.openActions();
    await this.action('Edit flight').click();
  }

  /** Removes the flight, confirmation and all. */
  async remove(): Promise<void> {
    await this.openActions();
    await this.action('Remove flight').click();
    await this.page
      .getByRole('dialog')
      .getByRole('button', { name: 'Remove' })
      .click();
  }

  /** Follows the download link and hands back the file the browser got. */
  async downloadSource(): Promise<Download> {
    await this.openActions();
    const download = this.page.waitForEvent('download');
    await this.action('Download original track').click();
    return download;
  }

  private openActions(): Promise<void> {
    return this.page.getByRole('button', { name: 'Flight actions' }).click();
  }

  /** A menu entry — portalled to the body, so it's looked up on the page. */
  private action(name: string): Locator {
    return this.page.getByRole('menuitem', { name });
  }
}
