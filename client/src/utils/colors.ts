/**
 * Parse a `#rrggbb` hex string plus a `0..1` opacity into an `[r, g, b, a]`
 * byte tuple — the shape deck.gl's `getColor` accessor expects. Leading `#` is
 * optional; no shorthand (`#rgb`) support.
 */
export const hexToRgba = (
  hex: string,
  opacity: number,
): [number, number, number, number] => {
  const value = hex.startsWith('#') ? hex.slice(1) : hex;
  return [
    Number.parseInt(value.slice(0, 2), 16),
    Number.parseInt(value.slice(2, 4), 16),
    Number.parseInt(value.slice(4, 6), 16),
    Math.round(opacity * 255),
  ];
};
