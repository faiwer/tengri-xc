import styles from './FlightChart.module.scss';

/**
 * Placeholder for the smoothed-vario-over-time chart. The smoothing
 * pipeline already exists (`computeVario` in `track/varioSegments`); this
 * just needs a uPlot wiring similar to {@link AltitudeChart}, with the
 * coloured-bucket palette as fills.
 */
export function VarioChart() {
  return <div className={styles.placeholder}>Vario chart — coming soon</div>;
}
