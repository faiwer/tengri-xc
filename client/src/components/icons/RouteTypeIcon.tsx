import type { RouteType } from '../../api/tracks.io';
import { FaiTriangleIcon } from './FaiTriangleIcon';
import { FreeTriangleIcon } from './FreeTriangleIcon';
import { FreeDistanceIcon } from './FreeDistanceIcon';
import type { IconProps } from './icon';

export interface RouteTypeIconProps extends IconProps {
  kind: RouteType;
}

const ICON_BY_TYPE = {
  fai_triangle: FaiTriangleIcon,
  free_distance: FreeDistanceIcon,
  free_triangle: FreeTriangleIcon,
  task: FaiTriangleIcon,
} satisfies Record<RouteType, unknown>;

export function RouteTypeIcon({ kind, ...rest }: RouteTypeIconProps) {
  const Icon = ICON_BY_TYPE[kind];
  return <Icon {...rest} />;
}
