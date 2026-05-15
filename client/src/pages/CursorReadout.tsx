import { useMemo } from 'react';
import { usePreferences } from '../core/preferences';
import type { FlightAnalysis } from '../track/flightAnalysis';
import { formatShortTimeWithSeconds } from '../utils/formatDateTime';
import { formatSpeed, formatVario } from '../utils/formatUnits';
import styles from './TrackPage.module.scss';

interface CursorReadoutProps {
  analysis: FlightAnalysis | null;
  trackIndex: number | null;
}

export function CursorReadout({ analysis, trackIndex }: CursorReadoutProps) {
  const prefs = usePreferences();
  const readout = useMemo(
    () =>
      analysis && trackIndex !== null
        ? buildCursorReadout(analysis, trackIndex, prefs)
        : null,
    [analysis, trackIndex, prefs],
  );
  const fields = readout
    ? [
        ['Time', readout.time],
        ['GPS', readout.gps],
        ...(readout.baroAlt ? [['BaroAlt', readout.baroAlt]] : []),
        ['Vario', readout.vario],
        ['Speed', readout.speed],
      ]
    : [
        ['Time', '—'],
        ['GPS', '—'],
        ...(analysis?.track.baroAlt ? [['BaroAlt', '—']] : []),
        ['Vario', '—'],
        ['Speed', '—'],
      ];

  return (
    <div className={styles.cursorReadout}>
      {fields.map(([label, value]) => (
        <span key={label}>
          <span className={styles.cursorReadoutLabel}>{label}: </span>
          {value}
        </span>
      ))}
    </div>
  );
}

interface CursorReadoutValue {
  time: string;
  gps: string;
  baroAlt: string | null;
  vario: string;
  speed: string;
}

const buildCursorReadout = (
  analysis: FlightAnalysis,
  idx: number,
  prefs: ReturnType<typeof usePreferences>,
): CursorReadoutValue => {
  const { track, metrics } = analysis;
  return {
    time: formatShortTimeWithSeconds(
      track.t[idx]!,
      prefs,
      analysis.timeOffsetSeconds,
    ),
    gps: `${Math.round(track.alt[idx]! / 10).toLocaleString()} m`,
    baroAlt: track.baroAlt
      ? `${Math.round(track.baroAlt[idx]! / 10).toLocaleString()} m`
      : null,
    vario: formatVario(metrics.vario[idx]!, prefs),
    speed: formatSpeed(metrics.speed[idx]! / 3.6, prefs),
  };
};
