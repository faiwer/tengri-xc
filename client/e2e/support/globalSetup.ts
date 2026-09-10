import { seedSiteSettings } from '../fixtures/site';
import { tengri } from './tengri';

export default async function globalSetup(): Promise<void> {
  if (process.env.E2E_SKIP_DB_RESET === '1') {
    return;
  }

  await tengri(['migrate']);
  await tengri(['prune', '--yes']);
  await seedSiteSettings();
}
