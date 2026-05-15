import { cloneElement, type ElementNode } from 'react';

import type { GliderKindStaticIconProps } from './GliderKindIcon';
import { iconSvgStyle } from './gliderKindIconStyles';

const svg = (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    viewBox="64 64 896 896"
  >
    <g
      fill="currentColor"
      fillOpacity={1}
      stroke="none"
      strokeWidth={55}
      transform="translate(0 34.6)"
    >
      <path d="m802.784 431.973-51.928 58.557 66.844 46.956 119.324-59.11-34.803-34.25-66.843 28.726z" />
      <path d="m440.945 401.59-29.831-29.28 69.605-72.92 212.132-2.21 87.284 109.381-134.792 156.337 82.864 119.877-164.624 150.812-39.774-36.46 91.703-111.59c-33.81-50.697-92.19-70.124-97.228-157.442 3.193-39.482 24.456-63.673 46.404-87.283l64.634-72.92c-38.124-7.88-73.957-18.433-120.429-16.574Z" />
      <path d="M489.558 518.704c-1.283 46.6 2.503 87.878 30.384 103.856L310.019 826.958l-36.46-38.117Z" />
      <path d="M228.834 339.012v46.271h127.937l40-46.271zM88.137 438.449v46.27h371.937l40-46.27zm67.021 100.662v46.272h227.938l40-46.272zm70.46 100.934v46.271h79.937l40-46.271z" />
      <circle cx="840.5" cy="259.938" r="101.563" />
    </g>
  </svg>
) as ElementNode;

export function SpeedIcon({ className, style }: GliderKindStaticIconProps) {
  return cloneElement(svg, { className, style: iconSvgStyle(style) });
}
