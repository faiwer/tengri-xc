import { DingdingOutlined } from '@ant-design/icons';
import type { CSSProperties } from 'react';

import type { Sport } from '../../core/sport';
import { HgIcon } from './HgIcon';
import { PgIcon } from './PgIcon';
import { SpIcon } from './SpIcon';

export interface GliderKindStaticIconProps {
  className?: string;
  style?: CSSProperties;
}

export interface GliderKindIconProps extends GliderKindStaticIconProps {
  kind: Sport;
}

const ICON_BY_SPORT = {
  hg: HgIcon,
  pg: PgIcon,
  sp: SpIcon,
  other: DingdingOutlined,
} satisfies Record<Sport, unknown>;

export function GliderKindIcon({ kind, ...rest }: GliderKindIconProps) {
  const Icon = ICON_BY_SPORT[kind];
  return <Icon {...rest} />;
}
