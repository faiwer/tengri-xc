import type { ResolvedPreferences } from '../../core/preferences';

/**
 * Compact chart-axis time. Respects the user's 12h/24h preference, but drops
 * AM/PM on 12h labels so dense ticks stay short (`1:30`, not `1:30 PM`).
 * With an offset, render the flight-local wall clock like the metadata panel.
 */
const HOUR_MINUTE_H24_VIEWER = new Intl.DateTimeFormat(undefined, {
  hour: 'numeric',
  minute: '2-digit',
  hour12: false,
});

const HOUR_MINUTE_H24_UTC = new Intl.DateTimeFormat(undefined, {
  hour: 'numeric',
  minute: '2-digit',
  hour12: false,
  timeZone: 'UTC',
});

const HOUR_MINUTE_H12_VIEWER = new Intl.DateTimeFormat(undefined, {
  hour: 'numeric',
  minute: '2-digit',
  hour12: true,
});

const HOUR_MINUTE_H12_UTC = new Intl.DateTimeFormat(undefined, {
  hour: 'numeric',
  minute: '2-digit',
  hour12: true,
  timeZone: 'UTC',
});

export const formatHourMinute = (
  epochSeconds: number,
  timeFormat: ResolvedPreferences['timeFormat'],
  offsetSeconds?: number | null,
): string =>
  formatterFor(timeFormat, offsetSeconds)
    .formatToParts(new Date((epochSeconds + (offsetSeconds ?? 0)) * 1000))
    .filter(
      (part) =>
        part.type !== 'dayPeriod' &&
        !(part.type === 'literal' && part.value.trim() === ''),
    )
    .map((part) => part.value)
    .join('');

const formatterFor = (
  timeFormat: 'h12' | 'h24',
  offsetSeconds: number | null | undefined,
) => {
  if (timeFormat === 'h12') {
    return offsetSeconds == null ? HOUR_MINUTE_H12_VIEWER : HOUR_MINUTE_H12_UTC;
  }

  return offsetSeconds == null ? HOUR_MINUTE_H24_VIEWER : HOUR_MINUTE_H24_UTC;
};
