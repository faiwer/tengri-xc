import type { ReactNode } from 'react';

export interface CursorReadoutField {
  key: CursorReadoutKey;
  icon: ReactNode;
  tooltip: string;
  value: string;
  width: number | undefined;
}

export type CursorReadoutKey =
  | 'time'
  | 'gps'
  | 'baroAlt'
  | 'vario'
  | 'speed'
  | 'mapCenter';

export interface CursorReadoutValue {
  time: string;
  gps: string;
  baroAlt: string | null;
  vario: string;
  speed: string;
}

export interface CursorReadoutWidths {
  time: number;
  gps: number;
  baroAlt: number | undefined;
  vario: number;
  speed: number;
}
