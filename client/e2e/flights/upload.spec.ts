import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { flightFixturePath, seedGliders } from '../fixtures/flight';
import { seedUser } from '../fixtures/user';
import { FlightDetailsForm } from '../models/FlightDetailsForm';
import { FlightPage } from '../models/FlightPage';
import { Header } from '../models/Header';
import { LoginModal } from '../models/LoginModal';
import { UploadFlightModal } from '../models/UploadFlightModal';
import { dragFileOverPage } from '../support/dragFile';
import { makeId } from '../support/ids';

const FIXTURE = flightFixturePath('fai-T-110.3.igc');
const PASSWORD = 'thermals4days';
const FLIGHT_URL = /\/flight\/[\w-]+$/;

/** One of the two ways a flight file reaches the dialog. */
interface Handover {
  how: string;
  /** Each run seeds its own pilot, and display names are unique. */
  pilot: string;
  hand: (upload: UploadFlightModal, page: Page) => Promise<void>;
}

const HANDOVERS: Handover[] = [
  {
    how: 'picked through the file dialog',
    pilot: 'Dialog Pilot',
    hand: async (upload) => {
      await upload.open();
      await upload.pickFile(FIXTURE);
    },
  },
  {
    how: 'dragged onto the page',
    pilot: 'Dragging Pilot',
    hand: async (upload, page) => {
      // Dragging a file anywhere on the page is what opens the dialog, so this
      // path never touches the header button.
      const drag = await dragFileOverPage(page, FIXTURE);
      await expect(upload.root).toBeVisible();
      await drag.dropOn(upload.dropZone);
    },
  },
];

for (const handover of HANDOVERS) {
  test(`a pilot uploads a flight ${handover.how}`, async ({ page }) => {
    const pilot = await signInAsPilot(page, handover.pilot);
    const upload = new UploadFlightModal(page);
    const details = new FlightDetailsForm(page);

    await handover.hand(upload, page);

    // Approximated from the track alone, so the preview lands near the scored
    // numbers without matching them.
    await expect(upload.root).toContainText('110.0 km');
    await expect(upload.root).toContainText('120.5 km');
    await expect(upload.root).toContainText('03:33');

    await upload.continueToDetails();

    // Hang gliding is the discipline the form opens on, and this flight is one.
    await details.chooseGlider('Aeros', 'Combat');
    await details.chooseLaunch('Foot launch');
    await details.choosePropulsion('Free');
    await details.submit();

    // The upload answers once scoring has run, so the page it lands on has
    // everything on it.
    await expect(page).toHaveURL(FLIGHT_URL);
    const flight = new FlightPage(page);
    await expect(flight.meta).toContainText(pilot.name);
    await expect(flight.meta).toContainText('11 Aug 2025');
    await expect(flight.meta).toContainText('Aeros Combat');
    await expect(flight.meta).toContainText('110.3 km');
    await expect(flight.meta).toContainText('03:33');
  });
}

test('a pilot copies the glider from a previous flight', async ({ page }) => {
  await signInAsPilot(page, 'Copying Pilot');
  const upload = new UploadFlightModal(page);
  const details = new FlightDetailsForm(page);

  // Nothing to copy from until something has been uploaded, so the first
  // upload is this test's setup.
  await uploadFlight(page);
  const firstFlight = page.url();

  await upload.open();
  await upload.pickFile(FIXTURE);
  await upload.continueToDetails();

  // This time there's a step in between, offering what the last flight flew.
  await expect(upload.root).toContainText('Copy data from previous flights?');
  await upload.copyFrom('Aeros', 'Combat');

  // Which fills the details form, leaving nothing to do but submit.
  await details.submit();

  // A flight of its own, carrying the copied glider.
  await expect(page).not.toHaveURL(firstFlight);
  await expect(page).toHaveURL(FLIGHT_URL);
  await expect(new FlightPage(page).meta).toContainText('Aeros Combat');
});

test('a pilot removes their own flight', async ({ browser, page }) => {
  await signInAsPilot(page, 'Removing Pilot');
  await uploadFlight(page);
  const flightUrl = page.url();

  const flight = new FlightPage(page);
  await flight.remove();

  await expect(page.getByText('Flight removed')).toBeVisible();
  await expect(page).toHaveURL(/\/flights$/);

  const visitor = await browser.newPage();
  await visitor.goto(flightUrl);
  await expect(visitor.getByText("Couldn't load flight track")).toBeVisible();
  await expect(
    visitor.getByText('Could not download flight metadata'),
  ).toBeVisible();
  await visitor.close();
});

test('a pilot downloads the track they uploaded', async ({ page }) => {
  await signInAsPilot(page, 'Downloading Pilot');
  await uploadFlight(page);

  const download = await new FlightPage(page).downloadSource();

  // Ingest keeps the upload as it arrived, so what comes back is the file that
  // went in, byte for byte.
  expect(download.suggestedFilename()).toMatch(/^2025-08-11_\w+\.igc$/);
  expect(await sha256(await download.path())).toBe(await sha256(FIXTURE));
});

test('a pilot rewrites their flight details', async ({ page }) => {
  await signInAsPilot(page, 'Editing Pilot');
  await seedGliders('pg');
  await uploadFlight(page);

  const flight = new FlightPage(page);
  const details = new FlightDetailsForm(page);

  // Every field to a different value: the other discipline, a glider out of
  // its catalog, and another launch and propulsion.
  await flight.edit();
  await details.chooseDiscipline('pg');
  await details.chooseGlider('Nova', 'Mentor 3');
  await details.chooseLaunch('Winch');
  await details.choosePropulsion('Powered');
  await details.submit();

  await expect(page.getByText('Flight updated')).toBeVisible();
  await expect(flight.meta).toContainText('Nova Mentor 3');

  // The sidebar carries the glider but not how it launched, so the rest is
  // checked where it shows: the form, re-seeded from the stored flight.
  await page.reload();
  await expect(flight.meta).toContainText('Nova Mentor 3');
  await flight.edit();
  await expect(details.discipline('pg')).toBeChecked();
  await expect(details.root).toContainText('Winch');
  await expect(details.root).toContainText('Powered');
});

/** A pilot with the glider catalog the details form reads, already signed in. */
async function signInAsPilot(page: Page, name: string) {
  await seedGliders();
  const account = await seedUser({
    login: `uploader-${makeId()}`,
    name,
    email: `uploader-${makeId()}@example.test`,
    password: PASSWORD,
  });

  const header = new Header(page);
  const login = new LoginModal(page);
  await page.goto('/');
  await header.signIn.click();
  await login.signIn({ identifier: account.login, password: PASSWORD });
  await expect(header.signOut).toBeVisible();

  // Wait out the sign-in dialog's exit animation, or whatever looks for a
  // dialog next finds two of them.
  await expect(login.root).toBeHidden();

  return account;
}

/**
 * Uploads the fixture as a hang-glider flight and lands on its page — the
 * starting point for everything a pilot can do to a flight of their own.
 */
async function uploadFlight(page: Page): Promise<void> {
  const upload = new UploadFlightModal(page);
  const details = new FlightDetailsForm(page);

  await upload.open();
  await upload.pickFile(FIXTURE);
  await upload.continueToDetails();
  await details.chooseGlider('Aeros', 'Combat');
  await details.chooseLaunch('Foot launch');
  await details.choosePropulsion('Free');
  await details.submit();
  await expect(page).toHaveURL(FLIGHT_URL);
}

const sha256 = async (filePath: string): Promise<string> =>
  createHash('sha256')
    .update(await readFile(filePath))
    .digest('hex');
