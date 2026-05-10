import { createContext } from 'react';
import type { Me } from '../../api/users.io';

/**
 * Currently-logged-in user (`null` = anonymous), and a setter the login/logout
 * flows use to update it.
 *
 * `isLoading` is true until the boot `/users/me` probe resolves, so consumers
 * that need to distinguish "haven't asked yet" from "anonymous" (e.g.
 * owner-gated pages) can short-circuit on the loading state instead of
 * mis-reading it as anon.
 */
export interface IdentityContextValue {
  me: Me | null;
  isLoading: boolean;
  setMe: (me: Me | null) => void;
}

export const IdentityContext = createContext<IdentityContextValue | null>(null);
