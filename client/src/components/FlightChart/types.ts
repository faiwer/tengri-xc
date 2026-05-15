import { z } from 'zod';

export type ChartKind = 'altitude' | 'speed' | 'vario';

export const CHART_LABELS: Record<ChartKind, string> = {
  altitude: 'Altitude',
  speed: 'Speed',
  vario: 'Vario',
};

export const ACTIVE_KIND_SCHEMA = z.enum(['altitude', 'speed', 'vario']);

export const ACTIVE_KIND_STORAGE_OPTIONS = {
  schema: ACTIVE_KIND_SCHEMA,
  defaultValue: 'altitude' as ChartKind,
  strategy: 'initOnly' as const,
};
