import { expect, test, type Page } from '@playwright/test';
import { flightFixturePath, seedGliders } from '../fixtures/flight';
import { seedUser } from '../fixtures/user';
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

    await handover.hand(upload, page);

    // Approximated from the track alone, so the preview lands near the scored
    // numbers without matching them.
    await expect(upload.root).toContainText('110.0 km');
    await expect(upload.root).toContainText('120.5 km');
    await expect(upload.root).toContainText('03:33');

    await upload.continueToDetails();

    // Hang gliding is the discipline the form opens on, and this flight is one.
    await upload.chooseGlider('Aeros', 'Combat');
    await upload.chooseLaunch('Foot launch');
    await upload.choosePropulsion('Free');
    await upload.submit();

    // The upload answers once scoring has run, so the page it lands on has
    // everything on it.
    await expect(page).toHaveURL(FLIGHT_URL);
    const sidebar = page.getByRole('region', { name: 'Flight metadata' });
    await expect(sidebar).toContainText(pilot.name);
    await expect(sidebar).toContainText('11 Aug 2025');
    await expect(sidebar).toContainText('Aeros Combat');
    await expect(sidebar).toContainText('110.3 km');
    await expect(sidebar).toContainText('03:33');
  });
}

test('a pilot copies the glider from a previous flight', async ({ page }) => {
  await signInAsPilot(page, 'Copying Pilot');
  const upload = new UploadFlightModal(page);

  // Nothing to copy from until something has been uploaded, so the first
  // upload is this test's setup.
  await upload.open();
  await upload.pickFile(FIXTURE);
  await upload.continueToDetails();
  await upload.chooseGlider('Aeros', 'Combat');
  await upload.chooseLaunch('Foot launch');
  await upload.choosePropulsion('Free');
  await upload.submit();
  await expect(page).toHaveURL(FLIGHT_URL);
  const firstFlight = page.url();

  await upload.open();
  await upload.pickFile(FIXTURE);
  await upload.continueToDetails();

  // This time there's a step in between, offering what the last flight flew.
  await expect(upload.root).toContainText('Copy data from previous flights?');
  await upload.copyFrom('Aeros', 'Combat');

  // Which fills the details form, leaving nothing to do but submit.
  await upload.submit();

  // A flight of its own, carrying the copied glider.
  await expect(page).not.toHaveURL(firstFlight);
  await expect(page).toHaveURL(FLIGHT_URL);
  await expect(
    page.getByRole('region', { name: 'Flight metadata' }),
  ).toContainText('Aeros Combat');
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
