import type { ResolvedPreferences } from '../core/preferences';

// Pre-built formatters keyed by the relevant pref. `Intl.DateTimeFormat`
// construction is non-trivial; cache one per format so repeated calls
// don't allocate.

const DATE_FMT_DMY = new Intl.DateTimeFormat(undefined, {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
});

// `en-US` is the canonical month-day-year locale: forces M/D/Y order
// and `/` separators regardless of the page's locale, matching what
// US pilots expect.
const DATE_FMT_MDY = new Intl.DateTimeFormat('en-US', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
});

const TIME_FMT_H24 = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
});

const TIME_FMT_H12 = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  hour12: true,
});

/**
 * Short numeric date, ordered per the user's preference. `dd.mm.yyyy`
 * for dmy (German/Russian convention), `mm/dd/yyyy` for mdy (US).
 */
export const formatShortDate = (
  epochSeconds: number,
  prefs: Pick<ResolvedPreferences, 'dateFormat'>,
): string => {
  const fmt = prefs.dateFormat === 'mdy' ? DATE_FMT_MDY : DATE_FMT_DMY;
  return fmt.format(new Date(epochSeconds * 1000));
};

/**
 * `HH:mm` (24-hour) or `hh:mm AM/PM` (12-hour) per the user's
 * preference. Locale handles the AM/PM word and any locale-specific
 * separators.
 */
export const formatShortTime = (
  epochSeconds: number,
  prefs: Pick<ResolvedPreferences, 'timeFormat'>,
): string => {
  const fmt = prefs.timeFormat === 'h24' ? TIME_FMT_H24 : TIME_FMT_H12;
  return fmt.format(new Date(epochSeconds * 1000));
};

/** `HH:mm` flight duration. Unit-agnostic; not affected by preferences. */
export const formatDuration = (totalSeconds: number): string => {
  const totalMinutes = Math.floor(totalSeconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
};
