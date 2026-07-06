import clsx from 'clsx';
import { Marker } from 'react-map-gl/maplibre';
import type { LatLng } from '../../utils/geo/coordinates';
import styles from './Markers.module.scss';

interface TrackHoverMarkerProps {
  point: LatLng | null;
}

/**
 * Chart-linked cursor dot. Rendered as a DOM `<Marker>` (not a GL layer) so it
 * - tracks prop changes without going through the `styledata` re-registration
 *   dance;
 * - and sits above every canvas layer, including the deck.gl track and the
 *   route legs, without needing `beforeId`;
 */
export function TrackHoverMarker({ point }: TrackHoverMarkerProps) {
  if (!point) {
    return null;
  }

  return (
    <Marker longitude={point.lng} latitude={point.lat}>
      <div className={clsx(styles.marker, styles.hover)} />
    </Marker>
  );
}
