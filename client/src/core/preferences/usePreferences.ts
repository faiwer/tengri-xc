import { useContext } from 'react';
import { nullthrows } from '../../utils/nullthrows';
import { PreferencesContext } from './PreferencesContext';
import type { ResolvedPreferences } from './types';

/**
 * Resolved preferences for the current viewer. Throws if used outside
 * `<PreferencesProvider>` — silent locale-default fallback would hide a wiring
 * bug.
 */
export function usePreferences(): ResolvedPreferences {
  return nullthrows(
    useContext(PreferencesContext),
    'usePreferences must be used inside a <PreferencesProvider>',
  );
}
