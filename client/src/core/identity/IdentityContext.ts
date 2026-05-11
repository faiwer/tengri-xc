import { createContext } from 'react';
import type { Me } from '../../api/users.io';

/**
 * Currently-logged-in user (`null` = anonymous), and a setter the login/logout
 * flows use to update it.
 *
 * The boot `/users/me` probe drives a tri-state: `isLoading` until the request
 * resolves, then either `me` (success — `null` for anon) or `error` (network /
 * HTTP failure). Consumers gated on identity should short-circuit on
 * `isLoading` first, then offer a retry path on `error`, before falling
 * through to "anonymous".
 */
export interface IdentityContextValue {
  me: Me | null;
  isLoading: boolean;
  /** Last boot-probe failure, or `null` if the probe succeeded. */
  error: unknown;
  /** Re-run the boot probe. No-op while one is already in flight. */
  retry: () => void;
  setMe: (me: Me | null) => void;
}

export const IdentityContext = createContext<IdentityContextValue | null>(null);
