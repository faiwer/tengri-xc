import type { Track } from '../track';
import clsx from 'clsx';
import type { FlightAnalysis } from '../track/flightAnalysis';
import styles from './MobileTrackInfo.module.scss';

interface Props {
  track: Track;
  analysis: FlightAnalysis;
  className?: string;
}

export function MobileTrackInfo({ track, analysis, className }: Props) {
  return (
    <div className={clsx(styles.panel, className)}>
      {track.lat.length}
      {analysis.vario.peakClimb}
    </div>
  );
}
