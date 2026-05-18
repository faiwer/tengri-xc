import { QuestionCircleOutlined } from '@ant-design/icons';
import { Tooltip } from 'antd';
import { useMemo } from 'react';
import type { TrackMetadata } from '../../api/tracks.io';
import styles from './TrackMetaPanel.module.scss';

interface LandingLabelProps {
  data: TrackMetadata;
}

export function LandingLabel({ data }: LandingLabelProps) {
  const tooltip = useMemo(() => {
    if (data.takeoffTimezone === data.landingTimezone) {
      return null;
    }

    const shiftSeconds = data.landingOffset - data.takeoffOffset;
    return `Timezone changed from ${data.takeoffTimezone} to ${data.landingTimezone}, shifting landing time by ${formatOffsetShift(shiftSeconds)}.`;
  }, [
    data.landingOffset,
    data.landingTimezone,
    data.takeoffOffset,
    data.takeoffTimezone,
  ]);

  return (
    <span className={styles.labelWithIcon}>
      Landing
      {tooltip && (
        <Tooltip title={tooltip}>
          <span
            className={styles.helpIcon}
            role="img"
            aria-label="Explain landing timezone"
          >
            <QuestionCircleOutlined />
          </span>
        </Tooltip>
      )}
    </span>
  );
}

function formatOffsetShift(seconds: number): string {
  const sign = seconds >= 0 ? '+' : '-';
  const absSeconds = Math.abs(seconds);
  const hours = Math.floor(absSeconds / 3600);
  const minutes = Math.floor((absSeconds % 3600) / 60);
  return minutes === 0
    ? `${sign}${hours}h`
    : `${sign}${hours}h ${minutes.toString().padStart(2, '0')}m`;
}
