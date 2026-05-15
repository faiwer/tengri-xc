// oxlint-disable react/style-prop-object -- SVG
import { cloneElement, type ElementNode } from 'react';

import type { GliderKindStaticIconProps } from './GliderKindIcon';
import { iconSvgStyle } from './gliderKindIconStyles';

// prettier-ignore
const svg = (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    viewBox="64 64 896 896"
  >
    <g fill="currentColor" fillOpacity={1} stroke="none" strokeOpacity={0.19837}>
      <path d="m700.249 122.744-153.82 153.82c-2.904 3.545-4.467 7.282-.244 11.845l29.35 29.352c2.09 1.893 5.37 3.91 10.002.351l93.497-93.499c1.828-.432 3.843-3.568 5.223 2.5V483.69c-.132 1.864.209 3.361 1.292 4.285l-.054.997h52.997a9.4 9.4 0 0 0 2.463-.06h2.158l-.049-.886c1.077-.924 1.42-2.417 1.287-4.277V227.172c1.38-6.068 3.395-2.932 5.223-2.5l93.497 93.5c4.633 3.558 7.912 1.541 10.001-.352l29.351-29.351c4.223-4.564 2.66-8.3-.244-11.846l-153.82-153.82c-4.549-6.403-25.013-6.554-28.11-.06M702.41 906.906l-153.82-153.82c-2.904-3.546-4.466-7.283-.244-11.846l29.351-29.351c2.09-1.894 5.37-3.91 10.002-.352l93.497 93.5c1.828.431 3.843 3.567 5.222-2.5V545.957c-.132-1.864.21-3.361 1.293-4.285l-.054-.997h52.996a9.4 9.4 0 0 1 2.464.06h2.158l-.049.886c1.077.924 1.419 2.417 1.287 4.277v256.578c1.38 6.068 3.395 2.932 5.223 2.5l93.497-93.5c4.632-3.558 7.912-1.541 10.001.353l29.351 29.35c4.222 4.564 2.66 8.301-.244 11.846l-153.82 153.82c-4.55 6.403-25.013 6.554-28.11.06" />
      <rect width="293.239" height="83.032" x="157.467" y="144.611" ry="13.663" />
      <rect width="215.988" height="83.032" x="157.536" y="306.223" ry="13.663" />
      <rect width="293.239" height="83.032" x="157.543" y="468.076" ry="13.663" />
      <rect width="293.239" height="83.032" x="156.112" y="792.721" ry="13.663" />
      <rect width="215.988" height="83.032" x="157.611" y="629.687" ry="13.663" />
    </g>
  </svg>
) as ElementNode;

export function VarioIcon({ className, style }: GliderKindStaticIconProps) {
  return cloneElement(svg, { className, style: iconSvgStyle(style) });
}
