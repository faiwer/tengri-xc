/**
 * Format an ISO 3166-1 alpha-2 country code (e.g. "DE") into a flag
 * emoji and a localized display name. Returns `null` for malformed
 * codes or codes the runtime doesn't recognize.
 *
 * @example
 * formatCountry('DE'); // { flag: '🇩🇪', name: 'Germany' }
 * formatCountry('xx'); // null (not uppercase)
 * formatCountry('ZZ'); // null (not assigned)
 */

const REGION_NAMES = new Intl.DisplayNames(undefined, {
  type: 'region',
  fallback: 'none',
});

const ALPHA2 = /^[A-Z]{2}$/;
const ASCII_A = 'A'.charCodeAt(0);
// First regional-indicator codepoint (🇦). The 26 letters are
// contiguous from there, so `0x1F1E6 + (ord(c) - ord('A'))` gives
// the matching indicator for any uppercase ASCII letter.
const REGIONAL_INDICATOR_A = 0x1f1e6;

export interface FormattedCountry {
  flag: string;
  name: string;
}

export const formatCountry = (code: string): FormattedCountry | null => {
  if (!ALPHA2.test(code)) {
    return null;
  }

  const flag = String.fromCodePoint(
    ...Array.from(
      code,
      (c) => c.charCodeAt(0) - ASCII_A + REGIONAL_INDICATOR_A,
    ),
  );
  const name = REGION_NAMES.of(code);
  return !name ? null : { flag, name };
};
