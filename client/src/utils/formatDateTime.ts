import type { ResolvedPreferences } from '../core/preferences';

// Two parallel formatter caches: one in viewer-TZ (no offset supplied,
// `Intl.DateTimeFormat` uses the browser's resolved zone) and one in `UTC` for
// the offset-supplied path. With an offset, we shift the instant by
// `offsetSeconds` *before* handing it to a `timeZone: 'UTC'` formatter — that's
// how we render flight-local wall-clock without having to know the IANA name on
// the client.
//
// `Intl.DateTimeFormat` construction is non-trivial; cache one per (preference
// value, has-offset?) so repeated render calls don't allocate.

// Order is locale-driven (`en-GB` → D/M/Y, `en-US` → M/D/Y); the separator
// literal that the locale would emit (`/` or `-`) is discarded by
// `joinWithDots` so every short date renders as `dd.mm.yyyy` / `mm.dd.yyyy`.
// With `undefined` instead of a fixed locale we'd inherit the browser's locale
// order, which on en-US would silently swap dmy → mdy.
const DATE_FMT_DMY_VIEWER = new Intl.DateTimeFormat('en-GB', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
});

const DATE_FMT_DMY_UTC = new Intl.DateTimeFormat('en-GB', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
  timeZone: 'UTC',
});

const DATE_FMT_MDY_VIEWER = new Intl.DateTimeFormat('en-US', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
});

const DATE_FMT_MDY_UTC = new Intl.DateTimeFormat('en-US', {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
  timeZone: 'UTC',
});

const TIME_FMT_H24_VIEWER = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
});

const TIME_FMT_H24_UTC = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
  timeZone: 'UTC',
});

const TIME_FMT_H12_VIEWER = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  hour12: true,
});

const TIME_FMT_H12_UTC = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  hour12: true,
  timeZone: 'UTC',
});

const TIME_SECONDS_FMT_H24_VIEWER = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: false,
});

const TIME_SECONDS_FMT_H24_UTC = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: false,
  timeZone: 'UTC',
});

const TIME_SECONDS_FMT_H12_VIEWER = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: true,
});

const TIME_SECONDS_FMT_H12_UTC = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
  hour12: true,
  timeZone: 'UTC',
});

const VERBOSE_DATE_FMT_VIEWER = new Intl.DateTimeFormat(undefined, {
  year: 'numeric',
  month: 'short',
  day: 'numeric',
});

const VERBOSE_DATE_FMT_UTC = new Intl.DateTimeFormat(undefined, {
  year: 'numeric',
  month: 'short',
  day: 'numeric',
  timeZone: 'UTC',
});

/**
 * Build a `Date` for the given UTC epoch and optional offset. With no offset,
 * returns the raw epoch instant (formatters then render it in the viewer's
 * zone). With an offset, shifts the instant forward by the offset so that a
 * `timeZone: 'UTC'` formatter prints the flight-local wall clock. Two
 * formatters per slot below — one for each path.
 */
const at = (epochSeconds: number, offsetSeconds: number | undefined): Date =>
  new Date((epochSeconds + (offsetSeconds ?? 0)) * 1000);

/**
 * Short numeric date, ordered per the user's preference. `dd.mm.yyyy` for dmy
 * (German/Russian convention), `mm/dd/yyyy` for mdy (US).
 *
 * Pass `offsetSeconds` to render in flight-local time (the date a pilot
 * remembers); omit to fall back to the viewer's TZ (useful for non-flight call
 * sites like account-creation timestamps).
 */
export const formatShortDate = (
  epochSeconds: number,
  prefs: Pick<ResolvedPreferences, 'dateFormat'>,
  offsetSeconds?: number,
): string => {
  const fmt =
    prefs.dateFormat === 'mdy'
      ? offsetSeconds === undefined
        ? DATE_FMT_MDY_VIEWER
        : DATE_FMT_MDY_UTC
      : offsetSeconds === undefined
        ? DATE_FMT_DMY_VIEWER
        : DATE_FMT_DMY_UTC;
  return joinWithDots(fmt.formatToParts(at(epochSeconds, offsetSeconds)));
};

/** Drop locale separators (`/`, `-`), keep numeric parts in their
 *  locale-chosen order, glue with `.`. */
const joinWithDots = (parts: Intl.DateTimeFormatPart[]): string =>
  parts
    .filter((p) => p.type !== 'literal')
    .map((p) => p.value)
    .join('.');

/**
 * `HH:mm` (24-hour) or `hh:mm AM/PM` (12-hour) per the user's preference.
 * Locale handles the AM/PM word and any locale-specific separators.
 *
 * `offsetSeconds` semantics match {@link formatShortDate}.
 */
export const formatShortTime = (
  epochSeconds: number,
  prefs: Pick<ResolvedPreferences, 'timeFormat'>,
  offsetSeconds?: number,
): string => {
  const fmt =
    prefs.timeFormat === 'h24'
      ? offsetSeconds === undefined
        ? TIME_FMT_H24_VIEWER
        : TIME_FMT_H24_UTC
      : offsetSeconds === undefined
        ? TIME_FMT_H12_VIEWER
        : TIME_FMT_H12_UTC;
  return fmt.format(at(epochSeconds, offsetSeconds));
};

/**
 * {@link formatShortTime}, but includes seconds for cursor/readout precision.
 */
export const formatShortTimeWithSeconds = (
  epochSeconds: number,
  prefs: Pick<ResolvedPreferences, 'timeFormat'>,
  offsetSeconds?: number,
): string => {
  const fmt =
    prefs.timeFormat === 'h24'
      ? offsetSeconds === undefined
        ? TIME_SECONDS_FMT_H24_VIEWER
        : TIME_SECONDS_FMT_H24_UTC
      : offsetSeconds === undefined
        ? TIME_SECONDS_FMT_H12_VIEWER
        : TIME_SECONDS_FMT_H12_UTC;
  return fmt.format(at(epochSeconds, offsetSeconds));
};

/**
 * Verbose locale-driven date ("May 3, 2026" / "3 May 2026"). Stays
 * locale-driven (no `dateFormat` honour) because that preference is about
 * *short numeric* dates; the verbose form picks its month-name language from
 * the locale and forcing en-US/en-GB to control ordering would also force
 * English month names on, say, German users.
 */
export const formatVerboseDate = (
  epochSeconds: number,
  offsetSeconds?: number,
): string => {
  const fmt =
    offsetSeconds === undefined
      ? VERBOSE_DATE_FMT_VIEWER
      : VERBOSE_DATE_FMT_UTC;
  return fmt.format(at(epochSeconds, offsetSeconds));
};

/** `HH:mm` flight duration. Unit-agnostic; not affected by preferences. */
export const formatDuration = (totalSeconds: number): string => {
  const totalMinutes = Math.floor(totalSeconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
};
