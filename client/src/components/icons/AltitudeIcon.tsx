// oxlint-disable react/style-prop-object -- SVG
import { cloneElement, type ElementNode } from 'react';

import { iconSvgStyle, type IconProps } from './icon';

// prettier-ignore
const svg = <svg xmlns="http://www.w3.org/2000/svg" aria-hidden="true" viewBox="64 64 896 896"><path d="M889.097 808.035H159.713L455.41 219.98l176.315 350.799 92.771-81.669z" style="fill:none;stroke:currentColor;stroke-width:65;"/></svg> as ElementNode;

export function AltitudeIcon({ className, style }: IconProps) {
  return cloneElement(svg, { className, style: iconSvgStyle(style) });
}
