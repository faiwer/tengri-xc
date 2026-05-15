// oxlint-disable react/style-prop-object -- SVG
import { cloneElement, type ElementNode, useRef } from 'react';

import type { GliderKindStaticIconProps } from './GliderKindIcon';
import { iconSvgStyle } from './gliderKindIconStyles';
import { useEffect } from 'react';
import { nullthrows } from '../../utils/nullthrows';
import { useState } from 'react';

const svg = (
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 62 62">
    <path
      d="M103.161 81.448c-8.7 0-15.751 7.052-15.751 15.751.02 7.83 5.77 14.388 13.394 15.535.02 0 3.11.383 4.756.03 7.644-1.174 13.35-7.76 13.352-15.565 0-8.7-7.052-15.751-15.751-15.751m.035 5.645c6.53 0 10.237 3.706 10.237 10.236s-3.707 10.237-10.237 10.237S92.96 103.86 92.96 97.33s3.706-10.237 10.237-10.237"
      style="fill:currentColor;stroke:none;stroke-width:.264583;fill-opacity:1"
      transform="translate(-71.806 -62.442)"
    />
    <g style="fill:currentColor;stroke-width:14.5521">
      <circle cx="49.982" cy="48.47" r="4.063" />
      <circle cx="55.146" cy="37.023" r="4.063" />
      <circle cx="53.332" cy="24.55" r="4.063" />
      <circle cx="44.677" cy="14.893" r="4.063" />
      <circle cx="31.203" cy="11.604" r="4.063" />
      <circle cx="17.747" cy="14.967" r="4.063" />
      <circle cx="9.336" cy="24.605" r="4.063" />
      <circle cx="7.257" cy="36.983" r="4.063" />
      <circle cx="12.529" cy="48.493" r="4.063" />
    </g>
  </svg>
) as ElementNode;

interface LoadingIconProps extends GliderKindStaticIconProps {
  // Set true when the icon is used on something dark.
  inverseTheme?: boolean;
}

export function LoadingIcon({
  className,
  style,
  inverseTheme = false,
}: LoadingIconProps) {
  const ref = useRef<SVGSVGElement>(null);
  const [visible, setVisible] = useState(false);
  const theme = inverseTheme ? INVERSE_THEME : DEFAULT_THEME;

  useEffect(() => {
    const timer = setTimeout(setVisible.bind(null, true), VISIBILITY_DELAY);
    return () => clearTimeout(timer);
  }, []);

  useEffect(() => {
    const svgElement = nullthrows(ref.current);
    const [middle, g] = [...svgElement.children] as SVGElement[];
    const count = g.children.length;
    const circles = [...g.children].reverse() as SVGCircleElement[];

    let iteration = 0;
    const interval = setInterval(() => {
      ++iteration;
      const stage = iteration % (count * 2 + 1);

      const activeStart = Math.max(0, stage - count);
      const activeEnd = Math.min(stage, count);
      const activeCount = activeEnd - activeStart;
      const brightness =
        1 + (theme.activeBrightness - 1) * (activeCount / count);

      middle.style.filter = `brightness(${brightness})`;

      for (const [idx, circle] of circles.entries()) {
        const active = activeStart <= idx && idx < activeEnd;
        circle.style.color = active ? theme.activeColor : '';
      }
    }, INTERVAL);

    return () => clearInterval(interval);
  }, [theme]);

  return cloneElement(svg, {
    className,
    style: {
      ...iconSvgStyle(style),
      color: theme.inactiveColor,
      visibility: visible ? '' : 'hidden',
    },
    ref,
  });
}

interface LoadingIconTheme {
  inactiveColor: string;
  activeColor: string;
  activeBrightness: number;
}

const DEFAULT_THEME: LoadingIconTheme = {
  inactiveColor: 'silver',
  activeColor: '#0f172a',
  activeBrightness: 0.25,
};

const INVERSE_THEME: LoadingIconTheme = {
  inactiveColor: '#333a4a',
  activeColor: '#7b8293',
  activeBrightness: 1.7,
};

const INTERVAL = 200;
const VISIBILITY_DELAY = 300;
