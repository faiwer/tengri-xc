import path from 'node:path';
import { repoRoot, tengri } from '../support/tengri';

const FLIGHT_FIXTURES_DIR = path.join(repoRoot, 'client/e2e/flights');
const E2E_USER_ID = 1001;
const E2E_GLIDER_BRAND = 'icaro';
const E2E_GLIDER_KIND = 'hg';
const E2E_GLIDER_MODEL = 'laminar';
const createdUsers = new Set<number>();
let glidersImported = false;

export async function seedFlightFixture(
  name: string,
): Promise<{ flightId: string }> {
  await ensureUser();
  await seedGliders();

  const add = await tengri([
    'add',
    flightFixturePath(name),
    '--user-id',
    String(E2E_USER_ID),
    '--brand',
    E2E_GLIDER_BRAND,
    '--kind',
    E2E_GLIDER_KIND,
    '--model',
    E2E_GLIDER_MODEL,
  ]);
  const flightId = parseAddedFlightId(add.stdout);
  await tengri(['score', '--update-db', '--', flightId]);
  return { flightId };
}

/** A track file the browser can be handed, by its name in `client/e2e/flights`. */
export const flightFixturePath = (name: string): string =>
  path.join(FLIGHT_FIXTURES_DIR, name);

/**
 * Import the public glider catalog. Uploading through the UI needs it: the
 * details form picks a brand and model out of it, and the server refuses a
 * flight naming a glider nobody has.
 */
export async function seedGliders(): Promise<void> {
  if (!glidersImported) {
    await tengri([
      'import-gliders',
      '--kind',
      E2E_GLIDER_KIND,
      '--file',
      path.join(repoRoot, `server/data/${E2E_GLIDER_KIND}.json`),
    ]);
    glidersImported = true;
  }
}

async function ensureUser(): Promise<void> {
  if (!createdUsers.has(E2E_USER_ID)) {
    await tengri([
      'user',
      'add',
      '--id',
      String(E2E_USER_ID),
      '--name',
      `E2E Pilot ${E2E_USER_ID}`,
      '--if-absent',
    ]);
    createdUsers.add(E2E_USER_ID);
  }
}

function parseAddedFlightId(stdout: string): string {
  const match = stdout.match(/added flight (\S+)/);
  if (!match) {
    throw new Error(
      `Could not parse flight id from tengri add output:\n${stdout}`,
    );
  }
  return match[1];
}
