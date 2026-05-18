import styles from './AltitudeChart.module.scss';

const MISSING_ALTITUDE_TEXT = "The track file doesn't contain altitude points";

export const MissingAltitudeChart = () => (
  <div className={styles.empty}>{MISSING_ALTITUDE_TEXT}</div>
);
