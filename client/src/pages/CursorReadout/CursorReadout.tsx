import { Tooltip } from 'antd';
import { useMemo, type CSSProperties } from 'react';
import { usePreferences } from '../../core/preferences';
import type { FlightAnalysis } from '../../track/flightAnalysis';
import { formatCoordinates } from '../../utils/formatGeo';
import styles from '../TrackPage.module.scss';
import { field } from './fields';
import { buildCursorReadout, buildCursorReadoutWidths } from './readout';

interface CursorReadoutProps {
  analysis: FlightAnalysis | null;
  mapCenter: google.maps.LatLngLiteral | null;
  trackIndex: number | null;
}

export function CursorReadout({
  analysis,
  mapCenter,
  trackIndex,
}: CursorReadoutProps) {
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
          field(
            'mapCenter',
            'Map center coordinates',
            mapCenter ? formatCoordinates(mapCenter) : '—',
            MAP_CENTER_WIDTH,
          ),
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

const MAP_CENTER_WIDTH = '46.75100, 13.17860'.length;

const segmentWidthStyle = (
  width: number | undefined,
): CSSProperties | undefined =>
  // 1ch = the "0" character width. We use monospace font, so this is accurate.
  width === undefined ? undefined : { width: `calc(${width}ch + 1.35rem)` };
