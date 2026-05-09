import { createContext } from 'react';
import type { Me } from '../../api/users.io';

/**
 * Currently-logged-in user (`null` = anonymous), and a setter the
 * login/logout flows use to update it.
 */
export interface IdentityContextValue {
  me: Me | null;
  setMe: (me: Me | null) => void;
}

export const IdentityContext = createContext<IdentityContextValue | null>(null);
