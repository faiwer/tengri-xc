import type { CSSProperties } from 'react';

export interface IconProps {
  className?: string;
  style?: CSSProperties;
  'aria-hidden'?: boolean | 'true' | 'false';
  'aria-label'?: string;
  'aria-labelledby'?: string;
  role?: string;
}

export function iconSvgStyle(extra?: CSSProperties): CSSProperties {
  return {
    width: '1em',
    height: '1em',
    verticalAlign: '-0.125em',
    ...extra,
  };
}

export function iconSvgProps({
  className: _className,
  style: _style,
  ...props
}: IconProps): IconProps {
  const hasLabel = !!props['aria-label'] || !!props['aria-labelledby'];

  return {
    ...props,
    'aria-hidden': hasLabel ? undefined : (props['aria-hidden'] ?? true),
    role: hasLabel ? (props.role ?? 'img') : props.role,
  };
}
