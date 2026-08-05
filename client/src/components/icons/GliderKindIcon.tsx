import { DingdingOutlined } from '@ant-design/icons';

import {
  GLIDER_KIND_LABEL,
  GLIDER_KIND_LABEL_PLURAL,
  type Sport,
} from '../../core/sport';
import { HgIcon } from './HgIcon';
import type { IconProps } from './icon';
import { PgIcon } from './PgIcon';
import { SpIcon } from './SpIcon';
import { Tooltip } from 'antd';

export interface GliderKindIconProps extends IconProps {
  kind: Sport;
  tooltip?: 'plural' | 'singular' | 'none';
}

const ICON_BY_SPORT = {
  hg: HgIcon,
  pg: PgIcon,
  sp: SpIcon,
  other: DingdingOutlined,
} satisfies Record<Sport, unknown>;

export function GliderKindIcon({
  kind,
  tooltip = 'none',
  ...rest
}: GliderKindIconProps) {
  const Icon = ICON_BY_SPORT[kind];
  const label =
    tooltip === 'plural'
      ? GLIDER_KIND_LABEL_PLURAL[kind]
      : GLIDER_KIND_LABEL[kind];
  return tooltip !== 'none' ? (
    <Tooltip title={label}>
      <span>
        <Icon {...rest} />
      </span>
    </Tooltip>
  ) : (
    <Icon {...rest} />
  );
}
