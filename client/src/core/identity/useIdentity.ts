import { useContext } from 'react';
import { nullthrows } from '../../utils/nullthrows';
import { IdentityContext, type IdentityContextValue } from './IdentityContext';

/**
 * Read the current identity (and the setter to mutate it). Throws if
 * called outside an `<IdentityProvider>` — silent anonymous fallback
 * would hide a wiring bug.
 */
export function useIdentity(): IdentityContextValue {
  return nullthrows(
    useContext(IdentityContext),
    'useIdentity must be used inside an <IdentityProvider>',
  );
}
