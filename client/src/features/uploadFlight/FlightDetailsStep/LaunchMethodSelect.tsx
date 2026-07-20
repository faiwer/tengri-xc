import { Select } from 'antd';
import type { LaunchMethod } from '../../../api/flights.io';
import styles from './FlightDetailsStep.module.scss';

export function LaunchMethodSelect({
  value,
  onChange,
}: {
  value: LaunchMethod | null;
  onChange: (method: LaunchMethod) => void;
}) {
  return (
    <Select<LaunchMethod>
      className={styles.select}
      placeholder="Launch method"
      value={value ?? undefined}
      options={LAUNCH_OPTIONS}
      onChange={onChange}
    />
  );
}

const LAUNCH_OPTIONS: { value: LaunchMethod; label: string }[] = [
  { value: 'foot', label: 'Foot launch' },
  { value: 'winch', label: 'Winch' },
  { value: 'aerotow', label: 'Aerotow' },
];
