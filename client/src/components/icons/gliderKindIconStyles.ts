import type { CSSProperties } from 'react';

/**
 * Shared sizing + baseline tweak for the glider-kind silhouettes.
 * Inline (not a CSS class) so consumers don't need to import a SCSS
 * module just to drop an icon. The `-0.125em` vertical-align matches
 * what Antd's own `<AntIcon>` does internally so the icon sits on the
 * text baseline alongside other icons / labels.
 *
 * Width / height are passed via `style` rather than as SVG attributes —
 * the `@faiwer/react` SVG types reject string values like `"1em"` on
 * `width` / `height` props.
 */
export function iconSvgStyle(extra?: CSSProperties): CSSProperties {
  return {
    width: '1em',
    height: '1em',
    verticalAlign: '-0.125em',
    ...extra,
  };
}
