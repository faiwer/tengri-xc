import { ClockCircleOutlined, EnvironmentOutlined } from '@ant-design/icons';
import type { ReactNode } from 'react';
import { AltitudeIcon } from '../../components/icons/AltitudeIcon';
import { BaroAltitudeIcon } from '../../components/icons/BaroAltitudeIcon';
import { SpeedIcon } from '../../components/icons/SpeedIcon';
import { VarioIcon } from '../../components/icons/VarioIcon';
import type { CursorReadoutField, CursorReadoutKey } from './types';
import type { ChartHelpItem } from '../../components/FlightChart/ChartHelp';

const FIELD_ICONS: Record<CursorReadoutKey, ReactNode> = {
  time: <ClockCircleOutlined />,
  gps: <AltitudeIcon />,
  baroAlt: <BaroAltitudeIcon />,
  pathSpeed: <SpeedIcon />,
  tas: <SpeedIcon />,
  vario: <VarioIcon />,
  speed: <SpeedIcon />,
  mapCenter: <EnvironmentOutlined />,
};

export const field = (
  key: CursorReadoutKey,
  tooltip: ChartHelpItem | string,
  value: string,
  width?: number,
  color?: string,
): CursorReadoutField => ({
  key,
  color,
  icon: FIELD_ICONS[key],
  tooltip:
    typeof tooltip === 'string' ? (
      tooltip
    ) : (
      <div>
        <strong style={{ color: tooltip.color }}>{tooltip.label}</strong>
        <br />
        {tooltip.text}
      </div>
    ),
  value,
  width,
});
