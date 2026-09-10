import type { Locator } from '@playwright/test';

/**
 * An antd form control, by the `name` its `Form.Item` was given.
 *
 * `getByLabel` would be the natural choice, but antd renders the label's
 * association attribute as `htmlfor` rather than `for`, so the browser links
 * no label to its control and every input is nameless to assistive tech. The
 * field name does land on the input as `id`, which is what this leans on.
 */
export const findField = (scope: Locator, name: string): Locator =>
  scope.locator(`#${name}`);
