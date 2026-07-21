import { Select } from 'antd';
import type { Propulsion } from '../../../api/flights.io';
import styles from './FlightDetailsStep.module.scss';

export function PropulsionSelect({
  value,
  onChange,
}: {
  value: Propulsion | null;
  onChange: (propulsion: Propulsion) => void;
}) {
  return (
    <Select<Propulsion>
      className={styles.select}
      placeholder="Propulsion"
      value={value ?? undefined}
      options={PROPULSION_OPTIONS}
      onChange={onChange}
    />
  );
}

const PROPULSION_OPTIONS: { value: Propulsion; label: string }[] = [
  { value: 'free', label: 'Free' },
  { value: 'self_launch', label: 'Self-launch' },
  { value: 'powered', label: 'Powered' },
];
