// oxlint-disable react/style-prop-object -- Colored SVG
import { cloneElement, type ElementNode } from 'react';

import { iconSvgStyle, type IconProps } from './icon';

const svg = (
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
    <path
      d="m21.273 72.191 23.225-45.306"
      style="fill:#393939;fill-opacity:1;stroke:#dba27b;stroke-width:6;stroke-dasharray:none;stroke-opacity:1"
    />
    <path
      d="m55.425 26.512 24.034 47.046"
      style="fill:none;fill-opacity:1;stroke:#dba27b;stroke-width:6;stroke-dasharray:none;stroke-opacity:1"
    />
    <path
      d="M73.91 83.142H27.065"
      style="fill:#393939;fill-opacity:1;stroke:#dba27b;stroke-width:6;stroke-dasharray:none;stroke-opacity:1"
    />
    <ellipse
      cx="81.243"
      cy="81.381"
      rx="13.912"
      ry="13.339"
      style="fill:#fff;fill-opacity:1;stroke:#7bdb7b;stroke-width:0;stroke-opacity:1"
    />
    <path
      d="M81.53 65.703a15.92 15.92 0 0 0-15.92 15.92 15.92 15.92 0 0 0 15.92 15.921 15.92 15.92 0 0 0 15.92-15.92 15.92 15.92 0 0 0-15.92-15.92m0 5.45A10.47 10.47 0 0 1 92 81.623a10.47 10.47 0 0 1-10.47 10.47 10.47 10.47 0 0 1-10.47-10.47 10.47 10.47 0 0 1 10.47-10.47"
      style="fill:#cb7500;fill-opacity:1;stroke-width:18.4456"
    />
    <ellipse
      cx="18.273"
      cy="81.381"
      rx="13.912"
      ry="13.339"
      style="fill:#fff;fill-opacity:1;stroke:#7bdb7b;stroke-width:0;stroke-opacity:1"
    />
    <path
      d="M18.56 65.703a15.92 15.92 0 0 0-15.92 15.92 15.92 15.92 0 0 0 15.92 15.921 15.92 15.92 0 0 0 15.92-15.92 15.92 15.92 0 0 0-15.92-15.92m0 5.45a10.47 10.47 0 0 1 10.47 10.47 10.47 10.47 0 0 1-10.47 10.47 10.47 10.47 0 0 1-10.471-10.47 10.47 10.47 0 0 1 10.47-10.47"
      style="fill:#cb7500;fill-opacity:1;stroke-width:18.4456"
    />
    <ellipse
      cx="48.964"
      cy="17.352"
      rx="13.912"
      ry="13.339"
      style="fill:#fff;fill-opacity:1;stroke:#7bdb7b;stroke-width:0;stroke-opacity:1"
    />
    <path
      d="M49.251 1.674a15.92 15.92 0 0 0-15.92 15.92 15.92 15.92 0 0 0 15.92 15.921 15.92 15.92 0 0 0 15.92-15.92 15.92 15.92 0 0 0-15.92-15.92m0 5.45a10.47 10.47 0 0 1 10.47 10.47 10.47 10.47 0 0 1-10.47 10.47 10.47 10.47 0 0 1-10.47-10.47 10.47 10.47 0 0 1 10.47-10.47"
      style="fill:#cb7500;fill-opacity:1;stroke-width:18.4456"
    />
  </svg>
) as ElementNode;

export function FreeTriangleIcon({ className, style }: IconProps) {
  return cloneElement(svg, { className, style: iconSvgStyle(style) });
}
