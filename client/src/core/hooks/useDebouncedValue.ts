import { useEffect, useState } from 'react';

/**
 * Returns `value` after it stops changing for `delayMs`. Each new
 * `value` cancels the pending update and starts a fresh timer; the
 * returned reference only flips once the input settles.
 *
 * @example
 * const [query, setQuery] = useState('');
 * const debouncedQuery = useDebouncedValue(query, 250);
 *
 * useEffect(() => {
 *   fetchSearch(debouncedQuery);
 * }, [debouncedQuery]);
 */
export function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const handle = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(handle);
  }, [value, delayMs]);

  return debounced;
}
