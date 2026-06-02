import { expect, type Locator, type Page, test } from '@playwright/test';
import { seedFlightFixture } from './support/flightFixtures';

const COORDINATES_READOUT = /^[0-9]+\.[0-9]+,\s+[0-9]+\.[0-9]+$/;

test(`anonymous visitor can open a flight page`, async ({ page }) => {
  const { flightId } = await seedFlightFixture('fai-T-110.3.igc');
  await page.goto(`/flight/${flightId}`);

  const sidebar = page.getByRole('region', { name: 'Flight metadata' });
  await expect(sidebar).toBeVisible();

  await expect(sidebar).toContainText('176'); // score
  await expect(sidebar).toContainText('110.3 km');
  await expect(sidebar).toContainText('03:33'); // duration
  await expect(sidebar).toContainText('14:09'); // start time
  await expect(sidebar).toContainText('17:42'); // landing time
  await expect(sidebar).toContainText('11 Aug 2025');
  await expect(sidebar).toContainText('Icaro Laminar');
  await expect(sidebar).toContainText('+5.5 m/s');
  await expect(sidebar).toContainText('−5.2 m/s');
  await expect(sidebar).toContainText('4.219 m');
  await expect(sidebar).toContainText('881 m');
});

test('map hover updates the cursor readout', async ({ page }) => {
  const { flightId } = await seedFlightFixture('fai-T-110.3.igc');
  await page.goto(`/flight/${flightId}`);

  const readout = page.getByRole('status', { name: 'Cursor readout' });
  await expect(readout).toBeVisible();

  const map = page.getByTestId('flight-map');
  await expect(map).toBeVisible();
  const box = await map.boundingBox();
  if (!box) {
    throw new Error('Flight map has no bounding box');
  }

  await map.hover({
    position: { x: box.width / 2, y: box.height / 2 },
  });

  await checkCursorReadout(page, readout);
});

test('chart hover updates the cursor readout', async ({ page }) => {
  const { flightId } = await seedFlightFixture('fai-T-110.3.igc');
  await page.goto(`/flight/${flightId}`);

  const readout = page.getByRole('status', { name: 'Cursor readout' });
  await expect(readout).toBeVisible();

  const chart = page.getByTestId('flight-chart');
  await expect(chart).toBeVisible();
  const box = await chart.boundingBox();
  if (!box) {
    throw new Error('Flight chart has no bounding box');
  }

  await chart.hover({
    position: { x: box.width / 2, y: box.height / 2 },
  });

  await checkCursorReadout(page, readout);
});

test('chart help tooltip follows the selected chart', async ({ page }) => {
  const { flightId } = await seedFlightFixture('fai-T-110.3.igc');
  await page.goto(`/flight/${flightId}`);

  const chartPanel = page.getByRole('region', { name: 'Flight charts' });
  await expect(chartPanel).toBeVisible();

  await checkChartHelpTooltip(page, chartPanel, 'better for absolute height');

  await chartPanel.getByRole('img', { name: 'Speed' }).click();
  await checkChartHelpTooltip(page, chartPanel, 'cross-country ground speed');

  await chartPanel.getByRole('img', { name: 'Vario' }).click();
  await checkChartHelpTooltip(page, chartPanel, 'positive vertical speed');
});

async function checkChartHelpTooltip(
  page: Page,
  chartPanel: Locator,
  expectedText: string,
) {
  await chartPanel
    .getByRole('img', { name: 'Explain active chart lines' })
    .hover();
  await expect(page.getByRole('tooltip')).toContainText(expectedText);
  await moveMouseToPageCorner(page);
}

async function checkCursorReadout(page: Page, readout: Locator) {
  await expect(readout).toContainText('m');
  await expect(readout).toContainText('m/s');
  await expect(readout).toContainText('km/h');

  await readout
    .getByText(/^[0-9,.]+ m$/)
    .first()
    .hover();
  const tooltip = page.getByRole('tooltip');
  await expect(tooltip).toContainText('GPS');
  await expect(tooltip).toContainText(
    'better for absolute height, noisier for altitude differences',
  );

  await moveMouseToPageCorner(page);

  await expect(readout).toHaveText(COORDINATES_READOUT);
  await expect(readout).not.toContainText('m/s');
  await expect(readout).not.toContainText('km/h');
}

async function moveMouseToPageCorner(page: Page) {
  const viewport = page.viewportSize();
  if (!viewport) {
    throw new Error('Page has no viewport size');
  }

  await page.mouse.move(viewport.width - 1, 1);
}
