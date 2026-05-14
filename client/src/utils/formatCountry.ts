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
let cachedCountryOptions: CountryOption[] | null = null;
// prettier-ignore
const DEPRECATED_REGION_CODES = new Set(['AN', 'BU', 'CS', 'DD', 'DY', 'FX', 'HV', 'NH', 'NT', 'RH', 'SU', 'TP', 'VD', 'YD', 'YU', 'ZR']);
// prettier-ignore
const EXCEPTIONAL_REGION_CODES = new Set(['AC', 'CP', 'DG', 'EA', 'IC', 'TA', 'UK']);
const MACRO_REGION_CODES = new Set(['EU', 'EZ', 'QO', 'UN', 'ZZ']);
const PRIVATE_USE_REGION = /^X[A-Z]$/;

export interface FormattedCountry {
  flag: string;
  name: string;
}

export interface CountryOption extends FormattedCountry {
  code: string;
  label: string;
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

export const countryOptions = (): CountryOption[] => {
  if (cachedCountryOptions !== null) {
    return cachedCountryOptions;
  }

  const options: CountryOption[] = [];
  for (let i = 0; i < 26; i++) {
    for (let j = 0; j < 26; j++) {
      const code = String.fromCharCode(ASCII_A + i, ASCII_A + j);
      if (!isCountryOptionCode(code)) {
        continue;
      }

      const formatted = formatCountry(code);
      if (formatted !== null) {
        options.push({
          code,
          ...formatted,
          label: `${formatted.flag} ${formatted.name}`,
        });
      }
    }
  }

  return options.sort((a, b) => a.name.localeCompare(b.name));
};

const isCountryOptionCode = (code: string): boolean =>
  !DEPRECATED_REGION_CODES.has(code) &&
  !EXCEPTIONAL_REGION_CODES.has(code) &&
  !MACRO_REGION_CODES.has(code) &&
  !PRIVATE_USE_REGION.test(code);
