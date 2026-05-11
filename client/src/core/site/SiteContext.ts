import { createContext } from 'react';
import type { Site } from '../../api/site.io';

/**
 * Site-wide settings (header branding, footer link visibility, registration
 * toggle). One value, always populated — the provider starts with sane defaults
 * and overlays the server response when it lands, so consumers never deal with
 * a "loading" branch.
 *
 * `setSite` lets the admin form push fresh values after a save so the
 * header/footer update without a page reload.
 */
export interface SiteContextValue {
  site: Site;
  setSite: (site: Site) => void;
}

export const SiteContext = createContext<SiteContextValue | null>(null);
