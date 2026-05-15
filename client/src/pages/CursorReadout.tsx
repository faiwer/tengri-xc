import { ClockCircleOutlined } from '@ant-design/icons';
import { Tooltip } from 'antd';
import { useMemo, type CSSProperties, type ReactNode } from 'react';
import { usePreferences } from '../core/preferences';
import type { FlightAnalysis } from '../track/flightAnalysis';
import { formatShortTimeWithSeconds } from '../utils/formatDateTime';
import {
  formatAltitude,
  formatSpeed,
  formatVario,
  MPS_TO_KMH,
} from '../utils/formatUnits';
import styles from './TrackPage.module.scss';
import { VarioIcon } from '../components/icons/VarioIcon';
import { AltitudeIcon } from '../components/icons/AltitudeIcon';
import { BaroAltitudeIcon } from '../components/icons/BaroAltitudeIcon';
import { SpeedIcon } from '../components/icons/SpeedIcon';

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
  const fieldWidths = useMemo(
    () => (analysis ? buildCursorReadoutWidths(analysis, prefs) : null),
    [analysis, prefs],
  );
  const fields =
    readout && fieldWidths
      ? [
          field('time', 'Time', readout.time, fieldWidths.time),
          field('gps', 'GPS altitude', readout.gps, fieldWidths.gps),
          ...(readout.baroAlt
            ? [
                field(
                  'baroAlt',
                  'Barometric altitude',
                  readout.baroAlt,
                  fieldWidths.baroAlt,
                ),
              ]
            : []),
          field('vario', 'Vertical speed', readout.vario, fieldWidths.vario),
          field('speed', 'Ground speed', readout.speed, fieldWidths.speed),
        ]
      : [
          field('time', 'Time', '—', fieldWidths?.time),
          field('gps', 'GPS altitude', '—', fieldWidths?.gps),
          ...(analysis?.track.baroAlt
            ? [
                field(
                  'baroAlt',
                  'Barometric altitude',
                  '—',
                  fieldWidths?.baroAlt,
                ),
              ]
            : []),
          field('vario', 'Vertical speed', '—', fieldWidths?.vario),
          field('speed', 'Ground speed', '—', fieldWidths?.speed),
        ];

  return (
    <div className={styles.cursorReadout}>
      {fields.map(({ icon, key, tooltip, value, width }) => (
        <Tooltip key={key} title={tooltip}>
          <span
            className={styles.cursorReadoutSegment}
            style={segmentWidthStyle(width)}
            aria-label={tooltip}
          >
            <span className={styles.cursorReadoutIcon}>{icon}</span>
            <span className={styles.cursorReadoutValue}>{value}</span>
          </span>
        </Tooltip>
      ))}
    </div>
  );
}

interface CursorReadoutField {
  key: CursorReadoutKey;
  icon: ReactNode;
  tooltip: string;
  value: string;
  width: number | undefined;
}

type CursorReadoutKey = 'time' | 'gps' | 'baroAlt' | 'vario' | 'speed';

interface CursorReadoutValue {
  time: string;
  gps: string;
  baroAlt: string | null;
  vario: string;
  speed: string;
}

interface CursorReadoutWidths {
  time: number;
  gps: number;
  baroAlt: number | undefined;
  vario: number;
  speed: number;
}

const FIELD_ICONS: Record<CursorReadoutKey, ReactNode> = {
  time: <ClockCircleOutlined />,
  gps: <AltitudeIcon />,
  baroAlt: <BaroAltitudeIcon />,
  vario: <VarioIcon />,
  speed: <SpeedIcon />,
};

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
    gps: formatAltitude(track.alt[idx]! / 10, prefs),
    baroAlt: track.baroAlt
      ? formatAltitude(track.baroAlt[idx]! / 10, prefs)
      : null,
    vario: formatVario(metrics.vario[idx]!, prefs),
    speed: formatSpeed(metrics.speed[idx]! / MPS_TO_KMH, prefs),
  };
};

const buildCursorReadoutWidths = (
  analysis: FlightAnalysis,
  prefs: ReturnType<typeof usePreferences>,
): CursorReadoutWidths => {
  const { track, metrics, window, altitudes, vario } = analysis;
  const fromIdx = window.takeoffIdx;
  const toIdx = window.landingIdx + 1;
  const baroAltRange = track.baroAlt
    ? range(track.baroAlt, fromIdx, toIdx, (dm) => dm / 10)
    : null;
  const maxSpeed = max(metrics.speed, fromIdx, toIdx);

  const widths: CursorReadoutWidths = {
    time: Math.max(
      formatShortTimeWithSeconds(
        track.t[fromIdx] ?? 0,
        prefs,
        analysis.timeOffsetSeconds,
      ).length,
      formatShortTimeWithSeconds(
        track.t[window.landingIdx] ?? 0,
        prefs,
        analysis.timeOffsetSeconds,
      ).length,
    ),
    gps: Math.max(
      formatAltitude(altitudes.minAlt, prefs).length,
      formatAltitude(altitudes.maxAlt, prefs).length,
    ),
    baroAlt: baroAltRange
      ? Math.max(
          formatAltitude(baroAltRange.min, prefs).length,
          formatAltitude(baroAltRange.max, prefs).length,
        )
      : undefined,
    vario: Math.max(
      formatVario(vario.peakSink, prefs).length,
      formatVario(vario.peakClimb, prefs).length,
    ),
    // Ground-speed metrics are stored as km/h; formatSpeed takes m/s.
    speed: formatSpeed(maxSpeed / MPS_TO_KMH, prefs).length,
  };

  return widths;
};

const field = (
  key: CursorReadoutKey,
  tooltip: string,
  value: string,
  width?: number,
): CursorReadoutField => ({
  key,
  icon: FIELD_ICONS[key],
  tooltip,
  value,
  width,
});

const max = (
  values: ArrayLike<number>,
  fromIdx: number,
  toIdx: number,
): number => {
  let result = 0;
  for (let idx = fromIdx; idx < toIdx; idx++) {
    const value = values[idx] ?? 0;
    if (value > result) {
      result = value;
    }
  }
  return result;
};

/** Finds the minimum and maximum values in a range of values. */
const range = (
  values: ArrayLike<number>,
  fromIdx: number,
  toIdx: number,
  /** E.g., m -> ft */
  convert: (value: number) => number,
): { min: number; max: number } => {
  if (fromIdx >= toIdx) {
    return { min: 0, max: 0 };
  }

  let minValue = convert(values[fromIdx] ?? 0);
  let maxValue = minValue;
  for (let idx = fromIdx + 1; idx < toIdx; idx++) {
    const value = convert(values[idx] ?? 0);
    if (value < minValue) {
      minValue = value;
    } else if (value > maxValue) {
      maxValue = value;
    }
  }

  return { min: minValue, max: maxValue };
};

/** Calculates the width of a segment based on the width of the value label. */
const segmentWidthStyle = (
  width: number | undefined,
): CSSProperties | undefined =>
  // 1ch = the "0" character width. We use monospace font, so this is accurate.
  width === undefined ? undefined : { width: `calc(${width}ch + 1.35rem)` };
