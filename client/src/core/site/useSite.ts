import { useContext } from 'react';
import { nullthrows } from '../../utils/nullthrows';
import { SiteContext, type SiteContextValue } from './SiteContext';

/**
 * Read site-wide settings + the setter used by the admin form. Throws if called
 * outside a `<SiteProvider>` — a silent default would hide a wiring bug.
 */
export function useSite(): SiteContextValue {
  return nullthrows(
    useContext(SiteContext),
    'useSite must be used inside a <SiteProvider>',
  );
}
