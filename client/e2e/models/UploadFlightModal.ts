import type { Locator, Page } from '@playwright/test';

/**
 * The upload dialog, from the drop zone through to the details form — which is
 * shared with the edit dialog and lives in `FlightDetailsForm`.
 */
export class UploadFlightModal {
  /** The dialog itself, for asserting on step titles and the preview. */
  root: Locator;
  dropZone: Locator;

  private page: Page;

  constructor(page: Page) {
    this.page = page;
    this.root = page.getByRole('dialog');
    this.dropZone = this.root.getByRole('button', {
      name: /Drop a flight file here/,
    });
  }

  /** Opens the dialog from the header, where a signed-in pilot starts. */
  open(): Promise<void> {
    return this.page.getByRole('button', { name: 'Upload flight' }).click();
  }

  /**
   * Picks a file through the dialog the drop zone opens on click. The listener
   * has to be waiting beforehand, or Playwright dismisses the chooser the
   * moment it appears.
   */
  async pickFile(filePath: string): Promise<void> {
    const chooser = this.page.waitForEvent('filechooser');
    await this.dropZone.click();
    await (await chooser).setFiles(filePath);
  }

  /** Leaves the preview for the rest of the form. */
  continueToDetails(): Promise<void> {
    return this.root.getByRole('button', { name: 'Continue' }).click();
  }

  /**
   * A row of the "copy data from previous flights" step. Brand and model sit
   * in separate spans, so they're matched one at a time rather than as one
   * string with a space in it.
   */
  previousFlight(brand: string, model: string): Locator {
    return this.root
      .getByRole('listitem')
      .filter({ hasText: brand })
      .filter({ hasText: model });
  }

  copyFrom(brand: string, model: string): Promise<void> {
    return this.previousFlight(brand, model).getByRole('button').click();
  }
}
