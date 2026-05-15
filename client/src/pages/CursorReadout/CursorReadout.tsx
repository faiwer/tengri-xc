import { Tooltip } from 'antd';
import { useMemo, type CSSProperties } from 'react';
import { chartHelpItems } from '../../components/FlightChart/ChartHelp';
import type { ChartKind } from '../../components/FlightChart';
import { usePreferences } from '../../core/preferences';
import type { FlightAnalysis } from '../../track/flightAnalysis';
import { formatCoordinates } from '../../utils/formatGeo';
import styles from '../TrackPage.module.scss';
import { buildFields } from './buildFields';
import { field } from './fields';
import { buildCursorReadout, buildCursorReadoutWidths } from './readout';

interface CursorReadoutProps {
  activeChartKind: ChartKind;
  analysis: FlightAnalysis | null;
  mapCenter: google.maps.LatLngLiteral | null;
  trackIndex: number | null;
}

export function CursorReadout({
  activeChartKind,
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
  const helpItems = analysis
    ? chartHelpItems(
        activeChartKind,
        !!analysis.track.baroAlt,
        !!analysis.track.tas,
      )
    : [];
  const fields =
    readout && fieldWidths
      ? buildFields(activeChartKind, readout, fieldWidths, helpItems)
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
      {fields.map(({ color, icon, key, tooltip, value, width }) => (
        <Tooltip key={key} title={tooltip}>
          <span
            className={styles.cursorReadoutSegment}
            style={segmentWidthStyle(width)}
          >
            <span
              className={styles.cursorReadoutIcon}
              style={iconColorStyle(color)}
            >
              {icon}
            </span>
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

const iconColorStyle = (color: string | undefined): CSSProperties | undefined =>
  color === undefined ? undefined : { color };
