import { useCallback, useState } from 'react';
import type { ZodType, infer as zInfer } from 'zod';
import { localStorageJson } from './localStorage';

/**
 * Persist a small piece of state in `localStorage` as JSON, validated
 * by a zod schema on read. Designed for view preferences (active tab,
 * panel collapsed/expanded, last-used filter) — anything where the
 * stored value is a leaf primitive or shallow object that should
 * survive a page reload.
 *
 * Behaviour with `strategy: 'initOnly'`:
 * - On first render, read the key from `localStorage`, parse as JSON,
 *   validate against `schema`. On any failure (missing key, malformed
 *   JSON, schema mismatch) fall back to `defaultValue` and remove the
 *   stale entry so subsequent renders don't keep re-failing.
 * - On `setValue`, update React state and write JSON to
 *   `localStorage`. No cross-tab `storage` event listener — the local
 *   state is the source of truth for the rest of the session, even if
 *   another tab writes a different value to the same key.
 *
 * Other strategies may be added later (e.g. `'sync'` to subscribe to
 * `storage` events). Keeping the strategy literal in the API today
 * means future additions are non-breaking.
 */
export interface UseLocalStorageValueOptions<S extends ZodType> {
  schema: S;
  defaultValue: zInfer<S>;
  strategy: 'initOnly';
}

export function useLocalStorageValue<S extends ZodType>(
  key: string,
  options: UseLocalStorageValueOptions<S>,
): [zInfer<S>, (value: zInfer<S>) => void] {
  const { schema, defaultValue } = options;

  const [value, setValue] = useState<zInfer<S>>(() =>
    localStorageJson.read(key, schema, defaultValue),
  );

  const setAndPersist = useCallback(
    (next: zInfer<S>) => {
      setValue(next);
      localStorageJson.write(key, next);
    },
    [key],
  );

  return [value, setAndPersist];
}
