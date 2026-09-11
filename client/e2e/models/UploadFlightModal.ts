import type { Locator, Page } from '@playwright/test';

/** The upload dialog, from the drop zone through to the details form. */
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

  async chooseGlider(brand: string, model: string): Promise<void> {
    await this.searchFor(BRAND, brand);
    await this.searchFor(MODEL, model);
  }

  chooseLaunch(method: string): Promise<void> {
    return this.choose(LAUNCH, method);
  }

  choosePropulsion(propulsion: string): Promise<void> {
    return this.choose(PROPULSION, propulsion);
  }

  submit(): Promise<void> {
    return this.root.getByRole('button', { name: 'Submit' }).click();
  }

  /**
   * One of the details form's four selects. antd gives them no accessible name
   * and keeps their labels in a sibling span, so they go by the order they're
   * rendered in.
   */
  private select(index: number): Locator {
    return this.root.getByRole('combobox').nth(index);
  }

  private async choose(index: number, option: string): Promise<void> {
    await this.select(index).click();
    await this.option(option).click();
  }

  /**
   * The glider lists run to hundreds of entries and render lazily, so typing
   * is what brings the wanted one into view. The click has to come first:
   * filling the search box on its own leaves the dropdown closed, and a
   * closed dropdown keeps its options in the DOM but hidden.
   */
  private async searchFor(index: number, option: string): Promise<void> {
    const select = this.select(index);
    await select.click();
    await select.fill(option);
    await this.option(option).click();
  }

  /**
   * A dropdown entry, addressed by the `title` antd puts on it — and pinned to
   * an entry, because the closed select shows its current value under the same
   * `title`.
   *
   * `getByRole('option')` doesn't work here: the dropdown is a virtual list,
   * and antd gives `role="option"` only to a hidden 0×0 mirror of the few
   * entries around the active one. The clickable entries have no role at all.
   * The lookup starts at the page because the dropdown is portalled to the
   * body, outside the dialog.
   */
  private option(name: string): Locator {
    return this.page
      .locator('.ant-select-item-option')
      .and(this.page.getByTitle(name, { exact: true }));
  }
}

const BRAND = 0;
const MODEL = 1;
const LAUNCH = 2;
const PROPULSION = 3;
