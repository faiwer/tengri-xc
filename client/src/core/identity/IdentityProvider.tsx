import { useMemo, useState, type ReactNode } from 'react';
import { getMe } from '../../api/users';
import type { Me } from '../../api/users.io';
import { trackError } from '../errors/trackError';
import { useAsyncEffect, useEventHandler } from '../hooks';
import { IdentityContext, type IdentityContextValue } from './IdentityContext';

interface IdentityProviderProps {
  children: ReactNode;
}

/**
 * Owns the `Me | null` state. Boots anonymous, then asks the server who we are
 * via `getMe()`: a stale cookie resolves to `null` (the server clears it for
 * us), a live one to the user record. The setter is exposed so login / logout
 * can update without a round-trip.
 *
 * If the boot probe fails (server down, network error, …) we land in an
 * explicit error state rather than leaving `isLoading` stuck at `true` — every
 * gated page would otherwise keep its skeleton spinning forever. `retry`
 * re-runs the probe in place.
 */
export function IdentityProvider({ children }: IdentityProviderProps) {
  const [me, setMe] = useState<Me | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);
  const [retryToken, setRetryToken] = useState(0);

  useAsyncEffect(
    async (signal) => {
      setIsLoading(true);
      setError(null);
      try {
        const next = await getMe({ signal });
        if (!signal.aborted) {
          setMe(next);
          setIsLoading(false);
        }
      } catch (err) {
        if (signal.aborted) return;
        trackError(err, { feature: 'identity', origin: 'IdentityProvider' });
        setError(err);
        setIsLoading(false);
      }
    },
    [retryToken],
  );

  const retry = useEventHandler(() => setRetryToken((t) => t + 1));

  return (
    <IdentityContext.Provider
      value={useMemo<IdentityContextValue>(
        () => ({ me, isLoading, error, retry, setMe }),
        [me, isLoading, error, retry],
      )}
    >
      {children}
    </IdentityContext.Provider>
  );
}
