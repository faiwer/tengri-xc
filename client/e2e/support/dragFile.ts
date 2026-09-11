import path from 'node:path';
import { readFile } from 'node:fs/promises';
import type { JSHandle, Locator, Page } from '@playwright/test';

/** A file being held over the page, waiting to be let go of. */
export interface FileDrag {
  /** Releases it over `target`, the way letting go of the mouse would. */
  dropOn(target: Locator): Promise<void>;
}

/**
 * Drag a file from outside the browser onto the page.
 *
 * A real OS drag can't be driven from Playwright, so this builds the
 * `DataTransfer` the browser would have handed over and fires the events the
 * app listens for itself: `dragenter` on the document, then `drop` wherever
 * the caller releases it.
 */
export async function dragFileOverPage(
  page: Page,
  filePath: string,
): Promise<FileDrag> {
  const dataTransfer = await buildDataTransfer(page, filePath);
  await page.dispatchEvent('body', 'dragenter', { dataTransfer });

  return {
    dropOn: (target) => target.dispatchEvent('drop', { dataTransfer }),
  };
}

async function buildDataTransfer(
  page: Page,
  filePath: string,
): Promise<JSHandle<DataTransfer>> {
  const contents = await readFile(filePath, 'utf8');

  return page.evaluateHandle(
    ([name, body]) => {
      const transfer = new DataTransfer();
      // Adding the file is what puts `Files` in `transfer.types` and gives the
      // page something to read the extension off.
      transfer.items.add(new File([body], name));
      return transfer;
    },
    [path.basename(filePath), contents],
  );
}
