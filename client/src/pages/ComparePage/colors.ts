/**
 * Per-flight colour palette for the compare view. Flight `i` gets
 * `PALETTE[i % PALETTE.length]` — distinct hues that stay readable on both the
 * terrain map and the dark sidebar.
 */
export const PALETTE = [
  '#dc2626', // red-600
  '#2563eb', // blue-600
  '#16a34a', // green-600
  '#f59e0b', // amber-500
  '#9333ea', // purple-600
  '#0891b2', // cyan-600
  '#db2777', // pink-600
  '#65a30d', // lime-600
] as const;

export const colorForIndex = (index: number): string =>
  PALETTE[index % PALETTE.length]!;
