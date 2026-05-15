import { ClockCircleOutlined, EnvironmentOutlined } from '@ant-design/icons';
import type { ReactNode } from 'react';
import { AltitudeIcon } from '../../components/icons/AltitudeIcon';
import { BaroAltitudeIcon } from '../../components/icons/BaroAltitudeIcon';
import { SpeedIcon } from '../../components/icons/SpeedIcon';
import { VarioIcon } from '../../components/icons/VarioIcon';
import type { CursorReadoutField, CursorReadoutKey } from './types';

const FIELD_ICONS: Record<CursorReadoutKey, ReactNode> = {
  time: <ClockCircleOutlined />,
  gps: <AltitudeIcon />,
  baroAlt: <BaroAltitudeIcon />,
  vario: <VarioIcon />,
  speed: <SpeedIcon />,
  mapCenter: <EnvironmentOutlined />,
};

export const field = (
  key: CursorReadoutKey,
  tooltip: string,
  value: string,
  width?: number,
): CursorReadoutField => ({
  key,
  icon: FIELD_ICONS[key],
  tooltip,
  value,
  width,
});
