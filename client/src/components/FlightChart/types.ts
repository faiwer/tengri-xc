export type ChartKind = 'altitude' | 'speed' | 'vario';

export const CHART_LABELS: Record<ChartKind, string> = {
  altitude: 'Altitude',
  speed: 'Speed',
  vario: 'Vario',
};
