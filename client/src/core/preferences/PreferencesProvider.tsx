import { useMemo, type ReactNode } from 'react';
import { useIdentity } from '../identity';
import { PreferencesContext } from './PreferencesContext';
import { resolvePreferences } from './resolve';

interface PreferencesProviderProps {
  children: ReactNode;
}

/**
 * Resolves the current viewer's preferences (or locale-derived defaults for
 * anonymous viewers) into concrete units and exposes them via
 * {@link PreferencesContext}.
 *
 * Lives in its own context — separate from identity — because pages available
 * to anon users (the public flights list, individual flight pages) want the
 * same formatting infra without claiming a user. The provider reads identity
 * when available, falls back to all-`'system'` resolution otherwise; consumers
 * stay agnostic.
 */
export function PreferencesProvider({ children }: PreferencesProviderProps) {
  const { me } = useIdentity();
  const resolved = useMemo(
    () => resolvePreferences(me?.preferences ?? null),
    [me?.preferences],
  );

  return (
    <PreferencesContext.Provider value={resolved}>
      {children}
    </PreferencesContext.Provider>
  );
}
