import { useMemo, useState, type ReactNode } from 'react';
import { getSite } from '../../api/site';
import type { Site } from '../../api/site.io';
import { useAsyncEffect } from '../hooks';
import { SiteContext, type SiteContextValue } from './SiteContext';

interface SiteProviderProps {
  children: ReactNode;
}

/**
 * Starts with these defaults so an offline / pre-fetch render shows the brand
 * and (optimistically) keeps the registration UI alive. Mirrors
 * `0005_site_settings.sql`'s column defaults.
 */
const FALLBACK_SITE: Site = {
  siteName: 'Tengri XC',
  canRegister: true,
  hasTos: false,
  hasPrivacy: false,
};

/**
 * Owns the site-settings state. Boots from {@link FALLBACK_SITE}, fetches
 * `/site` once, then overlays. Errors are swallowed — the fallback values are
 * good enough to render, and re-trying on every mount would just spam the
 * network with a public endpoint that rarely fails.
 */
export function SiteProvider({ children }: SiteProviderProps) {
  const [site, setSite] = useState<Site>(FALLBACK_SITE);

  useAsyncEffect(async (signal) => {
    try {
      const next = await getSite({ signal });
      if (!signal.aborted) setSite(next);
    } catch {
      // Keep the fallback. A logged-out visitor sees "Tengri XC" and a footer
      // with no doc links, which is the right degraded mode.
    }
  }, []);

  return (
    <SiteContext.Provider
      value={useMemo<SiteContextValue>(() => ({ site, setSite }), [site])}
    >
      {children}
    </SiteContext.Provider>
  );
}
