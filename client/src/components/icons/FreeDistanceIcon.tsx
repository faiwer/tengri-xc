// oxlint-disable react/style-prop-object -- Colored SVG
import { cloneElement, type ElementNode } from 'react';

import { iconSvgStyle, type IconProps } from './icon';

const POINT_COLOR = '#0033cb';
const LINE_COLOR = '#7bbadb';

const svg = (
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
    <path
      d="m18.232 82.593 15.384-49.627"
      style={`fill:#393939;fill-opacity:1;stroke:${LINE_COLOR};stroke-width:6;stroke-dasharray:none;stroke-opacity:1`}
    />
    <path
      d="m34.142 34.033 36.195 38.245"
      style={`fill:none;fill-opacity:1;stroke:${LINE_COLOR};stroke-width:6;stroke-dasharray:none;stroke-opacity:1`}
    />
    <path
      d="m71.51 71.94 10.122-54.567"
      style={`fill:#393939;fill-opacity:1;stroke:${LINE_COLOR};stroke-width:6;stroke-dasharray:none;stroke-opacity:1`}
    />
    <ellipse
      cx="70.66"
      cy="70.798"
      rx="13.912"
      ry="13.339"
      style="fill:#fff;fill-opacity:1;stroke:#7bdb7b;stroke-width:0;stroke-opacity:1"
    />
    <path
      d="M70.947 55.12a15.92 15.92 0 0 0-15.92 15.92 15.92 15.92 0 0 0 15.92 15.92 15.92 15.92 0 0 0 15.92-15.92 15.92 15.92 0 0 0-15.92-15.92m0 5.45a10.47 10.47 0 0 1 10.47 10.47 10.47 10.47 0 0 1-10.47 10.47 10.47 10.47 0 0 1-10.47-10.47 10.47 10.47 0 0 1 10.47-10.47"
      style={`fill:${POINT_COLOR};fill-opacity:1;stroke-width:18.4456`}
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
      style={`fill:${POINT_COLOR};fill-opacity:1;stroke-width:18.4456`}
    />
    <ellipse
      cx="33.089"
      cy="33.756"
      rx="13.912"
      ry="13.339"
      style="fill:#fff;fill-opacity:1;stroke:#7bdb7b;stroke-width:0;stroke-opacity:1"
    />
    <path
      d="M33.376 18.078a15.92 15.92 0 0 0-15.92 15.92 15.92 15.92 0 0 0 15.92 15.921A15.92 15.92 0 0 0 49.296 34a15.92 15.92 0 0 0-15.92-15.92m0 5.45a10.47 10.47 0 0 1 10.47 10.47 10.47 10.47 0 0 1-10.47 10.47 10.47 10.47 0 0 1-10.47-10.47 10.47 10.47 0 0 1 10.47-10.47"
      style={`fill:${POINT_COLOR};fill-opacity:1;stroke-width:18.4456`}
    />
    <ellipse
      cx="81.773"
      cy="17.352"
      rx="13.912"
      ry="13.339"
      style="fill:#fff;fill-opacity:1;stroke:#7bdb7b;stroke-width:0;stroke-opacity:1"
    />
    <path
      d="M82.06 1.674a15.92 15.92 0 0 0-15.92 15.92 15.92 15.92 0 0 0 15.92 15.921 15.92 15.92 0 0 0 15.92-15.92 15.92 15.92 0 0 0-15.92-15.92m0 5.45a10.47 10.47 0 0 1 10.47 10.47 10.47 10.47 0 0 1-10.47 10.47 10.47 10.47 0 0 1-10.471-10.47 10.47 10.47 0 0 1 10.47-10.47"
      style={`fill:${POINT_COLOR};fill-opacity:1;stroke-width:18.4456`}
    />
  </svg>
) as ElementNode;

export function FreeDistanceIcon({ className, style }: IconProps) {
  return cloneElement(svg, { className, style: iconSvgStyle(style) });
}
