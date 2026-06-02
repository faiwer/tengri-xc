import { expect, test } from '@playwright/test';
import { seedFlightFixture } from './support/flightFixtures';

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
