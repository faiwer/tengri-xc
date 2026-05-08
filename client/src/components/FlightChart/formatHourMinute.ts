/**
 * Format a Unix epoch (seconds) as `HH:mm` in the host locale, but
 * always 24h. uPlot's default formatter follows the OS clock setting,
 * which renders 12h with an "AM/PM" suffix on US-locale machines —
 * that suffix wastes horizontal space on dense flight-window axes
 * and reads as visual noise alongside the metadata panel's already-
 * 24h time strings.
 *
 * Implementation notes:
 * - `Intl.DateTimeFormat` doesn't expose a strict 24h flag in older
 *   runtimes (`hour12: false` works in modern browsers, but the
 *   `formatToParts` / dayPeriod-strip approach used here is portable
 *   and cheap), so we use `formatToParts` and drop the `dayPeriod`
 *   parts plus any whitespace literals around them.
 */
const HOUR_MINUTE_FORMATTER = new Intl.DateTimeFormat(undefined, {
  hour: 'numeric',
  minute: '2-digit',
});

export const formatHourMinute = (epochSeconds: number): string =>
  HOUR_MINUTE_FORMATTER.formatToParts(new Date(epochSeconds * 1000))
    .filter(
      (part) =>
        part.type !== 'dayPeriod' &&
        !(part.type === 'literal' && part.value.trim() === ''),
    )
    .map((part) => part.value)
    .join('');
