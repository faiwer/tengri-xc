import styles from './FlightChart.module.scss';

/**
 * Placeholder for the ground-speed-over-time chart. Once the data
 * pipeline lands (haversine between consecutive fixes, smoothed window),
 * this becomes a real uPlot panel similar in shape to {@link AltitudeChart}.
 */
export function SpeedChart() {
  return <div className={styles.placeholder}>Speed chart — coming soon</div>;
}
