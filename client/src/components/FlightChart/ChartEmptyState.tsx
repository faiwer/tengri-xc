import styles from './AltitudeChart.module.scss';

const MISSING_ALTITUDE_TEXT = "The track file doesn't contain altitude points";
const SPARSE_FIXES_TEXT =
  'The track points are too far apart to measure vertical speed';

export const MissingAltitudeChart = () => (
  <div className={styles.empty}>{MISSING_ALTITUDE_TEXT}</div>
);

export const SparseFixesChart = () => (
  <div className={styles.empty}>{SPARSE_FIXES_TEXT}</div>
);
