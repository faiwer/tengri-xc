import type { Locator, Page } from '@playwright/test';

/**
 * The discipline / glider / launch / propulsion form. The upload dialog's last
 * step and the "Edit flight" dialog render the same one, so it's addressed as
 * whichever dialog is open rather than as a part of either.
 */
export class FlightDetailsForm {
  /** The dialog holding the form, for asserting on the values it shows. */
  root: Locator;

  private page: Page;

  constructor(page: Page) {
    this.page = page;
    this.root = page.getByRole('dialog');
  }

  /**
   * Switches discipline, which clears the glider below it. The options are
   * bare icons, so they go by the order the form lists them, and the click
   * lands on the label wrapping antd's zero-sized radio.
   */
  chooseDiscipline(kind: Discipline): Promise<void> {
    return this.root
      .getByRole('radiogroup')
      .locator('label')
      .nth(DISCIPLINES.indexOf(kind))
      .click();
  }

  /** The radio behind a discipline option, for asserting which one is on. */
  discipline(kind: Discipline): Locator {
    return this.root
      .getByRole('radiogroup')
      .getByRole('radio')
      .nth(DISCIPLINES.indexOf(kind));
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

  /** The primary button — "Submit" when uploading, "Save" when editing. */
  submit(): Promise<void> {
    return this.root.getByRole('button', { name: /^(Submit|Save)$/ }).click();
  }

  /**
   * One of the form's four selects. antd gives them no accessible name and
   * keeps their labels in a sibling span, so they go by the order they're
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

/** Disciplines with a glider catalog, in the order the form lists them. */
const DISCIPLINES = ['hg', 'pg', 'sp'] as const;
type Discipline = (typeof DISCIPLINES)[number];

const BRAND = 0;
const MODEL = 1;
const LAUNCH = 2;
const PROPULSION = 3;
