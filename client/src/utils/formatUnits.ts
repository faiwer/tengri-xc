/**
 * Display formatters for the physical quantities the app surfaces: altitude,
 * vario, ground speed, distance. All inputs are in canonical SI units (metres,
 * m/s); the user's preferences decide what gets *shown*.
 *
 * Conversion + formatting live together because the rounding rule for "ft"
 * differs from the rounding rule for "m" (5 ft step vs 1 m step), and the chart
 * axes need both the converted number and the unit label as separate values.
 * The conversion-only helpers are exported for that case.
 */

import type { ResolvedPreferences } from '../core/preferences';

export const M_TO_FT = 3.28084;
/** 1 m/s = 3.28084 ft/s × 60 s/min = 196.8504 ft/min. */
export const MPS_TO_FPM = M_TO_FT * 60;
export const MPS_TO_KMH = 3.6;
export const MPS_TO_MPH = 2.23694;
export const M_TO_KM = 0.001;
export const M_TO_MI = 0.000_621_371;
/** km/h ↔ mph for series that are already in km/h (e.g. ground speed). */
export const KMH_TO_MPH = MPS_TO_MPH / MPS_TO_KMH;

export const metresToFeet = (m: number): number => m * M_TO_FT;
export const mpsToFpm = (mps: number): number => mps * MPS_TO_FPM;
export const mpsToKmh = (mps: number): number => mps * MPS_TO_KMH;
export const mpsToMph = (mps: number): number => mps * MPS_TO_MPH;
export const kmhToMph = (kmh: number): number => kmh * KMH_TO_MPH;

export const altitudeLabel = (
  prefs: Pick<ResolvedPreferences, 'units'>,
): string => (prefs.units === 'imperial' ? 'ft' : 'm');

export const varioLabel = (
  prefs: Pick<ResolvedPreferences, 'varioUnit'>,
): string => (prefs.varioUnit === 'fpm' ? 'ft/min' : 'm/s');

export const speedLabel = (
  prefs: Pick<ResolvedPreferences, 'speedUnit'>,
): string => (prefs.speedUnit === 'mph' ? 'mph' : 'km/h');

/**
 * Altitude, rounded to the nearest unit (1 m or 1 ft). Pilots are used to
 * instrument-precise readouts; coarser rounding makes peak altitudes look
 * stale.
 */
export const formatAltitude = (
  metres: number,
  prefs: Pick<ResolvedPreferences, 'units'>,
): string => {
  if (prefs.units === 'imperial') {
    return `${Math.round(metres * M_TO_FT).toLocaleString()} ft`;
  }
  return `${Math.round(metres).toLocaleString()} m`;
};

/**
 * Vario in m/s (one decimal) or ft/min (whole). The minus sign is the
 * typographic U+2212 to match a leading "+" optically; ASCII "-" sits too high.
 */
export const formatVario = (
  metresPerSec: number,
  prefs: Pick<ResolvedPreferences, 'varioUnit'>,
): string => {
  const sign = metresPerSec > 0 ? '+' : metresPerSec < 0 ? '−' : '';
  if (prefs.varioUnit === 'fpm') {
    const fpm = Math.round(Math.abs(metresPerSec) * MPS_TO_FPM);
    return `${sign}${fpm.toLocaleString()} ft/min`;
  }
  return `${sign}${Math.abs(metresPerSec).toFixed(1)} m/s`;
};

/** Ground speed, whole units (km/h or mph). */
export const formatSpeed = (
  metresPerSec: number,
  prefs: Pick<ResolvedPreferences, 'speedUnit'>,
): string => {
  const value =
    prefs.speedUnit === 'mph'
      ? metresPerSec * MPS_TO_MPH
      : metresPerSec * MPS_TO_KMH;
  return `${Math.round(value)} ${speedLabel(prefs)}`;
};

/**
 * Distance with auto-scaling: km/mi for ≥1 of that unit, m/ft below. Decimal
 * precision tightens at the long end (XC distance) and drops for the short end
 * (perimeter / waypoint offsets).
 */
export const formatDistance = (
  metres: number,
  prefs: Pick<ResolvedPreferences, 'units'>,
): string => {
  if (prefs.units === 'imperial') {
    const mi = metres * M_TO_MI;
    return mi >= 1
      ? `${mi.toFixed(1)} mi`
      : `${Math.round(metres * M_TO_FT).toLocaleString()} ft`;
  }

  const km = metres * M_TO_KM;
  return km >= 1
    ? `${km.toFixed(1)} km`
    : `${Math.round(metres).toLocaleString()} m`;
};
