import { createContext } from 'react';
import type { ResolvedPreferences } from './types';

/**
 * Resolved preferences for the current viewer (or browser-locale defaults for
 * anonymous viewers). Read with `usePreferences()`.
 */
export const PreferencesContext = createContext<ResolvedPreferences | null>(
  null,
);
