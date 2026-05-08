import type { ZodType, infer as zInfer } from 'zod';

/**
 * JSON-typed access to `localStorage` for view preferences (active tab,
 * panel collapsed/expanded, last-used filter) — anything where the
 * stored value is a leaf primitive or shallow object that should
 * survive a page reload.
 *
 * Keys are auto-prefixed with `tengri-` so the app's namespace inside
 * `localStorage` doesn't collide with anything else sharing the origin
 * (devtools extensions, embedded widgets). Callers pass the bare key
 * (`'flight-chart-tab'`); the prefix is an implementation detail.
 *
 * `localStorage` is assumed available — the app is browser-only and any
 * unavailability would be a real bug worth surfacing rather than
 * silently masking.
 */
export const localStorageJson = {
  /**
   * Read `key` from `localStorage`, parse as JSON, and validate against
   * `schema`. Returns the parsed value on success, `defaultValue` on
   * any failure (missing key, malformed JSON, schema mismatch).
   *
   * On malformed JSON or schema mismatch the entry is removed so the
   * next read doesn't repeat the same wasted parse — the stored value
   * is unrecoverable from the app's perspective and pretending
   * otherwise would just keep producing console noise.
   */
  read<S extends ZodType>(
    key: string,
    schema: S,
    defaultValue: zInfer<S>,
  ): zInfer<S> {
    const fullKey = prefix(key);
    const raw = localStorage.getItem(fullKey);
    if (raw === null) {
      return defaultValue;
    }

    // A stored value can be bad in two distinct ways (`JSON.parse`
    // SyntaxError, or `parsed` doesn't match the schema), and the
    // recovery is identical for both — drop the entry, fall back to
    // the default, move on.
    try {
      const parsed = JSON.parse(raw);
      const result = schema.safeParse(parsed);
      if (result.success) {
        return result.data;
      }
    } catch {
      // Malformed JSON — fall through to cleanup + default.
    }

    localStorage.removeItem(fullKey);
    return defaultValue;
  },

  /**
   * Write `value` to `localStorage` under `key` as JSON. No schema
   * validation on write — the type system already constrains the
   * caller, and re-validating an in-memory value the app just produced
   * would only catch programmer errors that are better caught by tests.
   */
  write<T>(key: string, value: T): void {
    localStorage.setItem(prefix(key), JSON.stringify(value));
  },
};

const prefix = (key: string): string => `tengri-${key}`;
