import { useMemo, useState, type ReactNode } from 'react';
import { getMe } from '../../api/users';
import type { Me } from '../../api/users.io';
import { useAsyncEffect } from '../hooks';
import { IdentityContext, type IdentityContextValue } from './IdentityContext';

interface IdentityProviderProps {
  children: ReactNode;
}

/**
 * Owns the `Me | null` state. Boots anonymous, then asks the server
 * who we are via `getMe()`: a stale cookie resolves to `null` (the
 * server clears it for us), a live one to the user record. The
 * setter is exposed so login / logout can update without a
 * round-trip.
 */
export function IdentityProvider({ children }: IdentityProviderProps) {
  const [me, setMe] = useState<Me | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useAsyncEffect(async (signal) => {
    const next = await getMe({ signal });
    if (!signal.aborted) {
      setMe(next);
      setIsLoading(false);
    }
  }, []);

  return (
    <IdentityContext.Provider
      value={useMemo<IdentityContextValue>(
        () => ({ me, isLoading, setMe }),
        [me, isLoading],
      )}
    >
      {children}
    </IdentityContext.Provider>
  );
}
