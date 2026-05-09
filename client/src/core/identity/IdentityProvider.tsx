import { useMemo, useState, type ReactNode } from 'react';
import type { Me } from '../../api/users.io';
import { IdentityContext, type IdentityContextValue } from './IdentityContext';

interface IdentityProviderProps {
  /** Initial value — `null` for anonymous. */
  value: Me | null;
  children: ReactNode;
}

/**
 * Owns the `Me | null` state and exposes it via `useIdentity`. The
 * setter is stable for the provider's lifetime (driven by `useState`),
 * so consumers can capture it without re-render concerns.
 */
export function IdentityProvider({ value, children }: IdentityProviderProps) {
  const [me, setMe] = useState<Me | null>(value);

  return (
    <IdentityContext.Provider
      value={useMemo<IdentityContextValue>(() => ({ me, setMe }), [me])}
    >
      {children}
    </IdentityContext.Provider>
  );
}
