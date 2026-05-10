import type { Preferences } from '../../api/users.io';
import type { ResolvedPreferences } from './types';

/**
 * Collapse every `'system'` literal to a concrete unit using the browser's
 * locale. Pure function — call from a `useMemo` keyed on the raw preferences
 * value.
 *
 * `null` input means "anonymous user, no saved preferences": each field is
 * treated as `'system'` and resolved from the locale.
 */
export function resolvePreferences(
  raw: Preferences | null,
): ResolvedPreferences {
  const units = resolveUnits(raw?.units ?? 'system');
  return {
    timeFormat: resolveTimeFormat(raw?.timeFormat ?? 'system'),
    dateFormat: resolveDateFormat(raw?.dateFormat ?? 'system'),
    units,
    // Vario/speed `'system'` defers to the resolved `units` choice so a metric
    // pilot doesn't get a mixed metric-altitude / imperial-vario surface from
    // leaving things on auto.
    varioUnit: resolveVarioUnit(raw?.varioUnit ?? 'system', units),
    speedUnit: resolveSpeedUnit(raw?.speedUnit ?? 'system', units),
    weekStart: resolveWeekStart(raw?.weekStart ?? 'system'),
  };
}

function resolveTimeFormat(raw: Preferences['timeFormat']): 'h12' | 'h24' {
  if (raw !== 'system') {
    return raw;
  }

  // `hour12` is reported as `false` for 24-hour locales, `true` (or a string
  // like `'h12'`) otherwise. Treat anything-not-false as 12-hour so
  // US/Canada/Australia/Mexico/etc. read correctly.
  const opts = new Intl.DateTimeFormat(undefined, {
    hour: 'numeric',
    minute: 'numeric',
  }).resolvedOptions();
  return opts.hour12 === false ? 'h24' : 'h12';
}

function resolveDateFormat(raw: Preferences['dateFormat']): 'dmy' | 'mdy' {
  if (raw !== 'system') {
    return raw;
  }

  // Format a date with distinguishable day/month and inspect part order.
  // `formatToParts` is the locale-correct way to ask "which comes first?"
  // without parsing a localised string.
  const parts = new Intl.DateTimeFormat(undefined, {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
  }).formatToParts(new Date(2026, 0, 15));
  const dayIdx = parts.findIndex((p) => p.type === 'day');
  const monthIdx = parts.findIndex((p) => p.type === 'month');
  return monthIdx >= 0 && dayIdx >= 0 && monthIdx < dayIdx ? 'mdy' : 'dmy';
}

// Regions that culturally use US customary or imperial units. Source: the
// Wikipedia "Metrication" article; in practice it's a four-region list (US,
// Liberia, Myanmar) plus the UK as a hybrid. The UK reports itself as `en-GB`;
// we err on the side of imperial because British pilots commonly expect feet
// for altitude.
const IMPERIAL_REGIONS = new Set(['US', 'GB', 'LR', 'MM']);

function resolveUnits(raw: Preferences['units']): 'metric' | 'imperial' {
  if (raw !== 'system') {
    return raw;
  }

  const region = new Intl.Locale(navigator.language).maximize().region;
  return region && IMPERIAL_REGIONS.has(region) ? 'imperial' : 'metric';
}

function resolveVarioUnit(
  raw: Preferences['varioUnit'],
  units: 'metric' | 'imperial',
): 'mps' | 'fpm' {
  if (raw !== 'system') {
    return raw;
  }
  return units === 'imperial' ? 'fpm' : 'mps';
}

function resolveSpeedUnit(
  raw: Preferences['speedUnit'],
  units: 'metric' | 'imperial',
): 'kmh' | 'mph' {
  if (raw !== 'system') {
    return raw;
  }
  return units === 'imperial' ? 'mph' : 'kmh';
}

// Locales whose calendar conventionally starts on Sunday. ECMA-402 is rolling
// out `Locale.prototype.getWeekInfo()`, but support is partial across browsers
// as of 2026 — we use it when available and fall back to a small known-Sunday
// list otherwise.
const SUNDAY_FIRST = new Set([
  'en-US',
  'en-CA',
  'pt-BR',
  'ja-JP',
  'he-IL',
  'ar-SA',
]);

function resolveWeekStart(raw: Preferences['weekStart']): 'mon' | 'sun' {
  if (raw !== 'system') {
    return raw;
  }

  const locale = new Intl.Locale(navigator.language);
  const info = locale as unknown as {
    getWeekInfo?: () => { firstDay: number };
    weekInfo?: { firstDay: number };
  };
  const firstDay = info.getWeekInfo?.().firstDay ?? info.weekInfo?.firstDay;
  if (firstDay) {
    // `firstDay`: 1 = Mon, 7 = Sun (ISO 8601 numbering).
    return firstDay === 7 ? 'sun' : 'mon';
  }

  return SUNDAY_FIRST.has(navigator.language) ? 'sun' : 'mon';
}
