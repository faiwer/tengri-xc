import type { CSSProperties } from 'react';

export interface IconProps {
  className?: string;
  style?: CSSProperties;
}

export function iconSvgStyle(extra?: CSSProperties): CSSProperties {
  return {
    width: '1em',
    height: '1em',
    verticalAlign: '-0.125em',
    ...extra,
  };
}
