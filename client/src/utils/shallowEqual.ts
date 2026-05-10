/**
 * Shallow equality for plain object records: same key set, same value at each
 * key (compared with `Object.is` so `NaN === NaN` and `+0 !== -0`). Doesn't
 * recurse — nested objects compare by reference.
 *
 * For form `isDirty` checks, settings diffs, prop-change guards on memoised
 * components — anywhere both inputs are flat records of primitives.
 */
export function shallowEqual<T extends Record<string, unknown>>(
  a: T,
  b: T,
): boolean {
  if (a === b) {
    return true;
  }

  const aKeys = Object.keys(a);
  if (aKeys.length !== Object.keys(b).length) {
    return false;
  }

  for (const key of aKeys) {
    if (!Object.is(a[key], b[key])) {
      return false;
    }
  }

  return true;
}
