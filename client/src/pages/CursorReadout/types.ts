import type { ReactNode } from 'react';

export interface CursorReadoutField {
  key: CursorReadoutKey;
  color: string | undefined;
  icon: ReactNode;
  tooltip: ReactNode;
  value: string;
  width: number | undefined;
}

export type CursorReadoutKey =
  | 'time'
  | 'gps'
  | 'baroAlt'
  | 'pathSpeed'
  | 'tas'
  | 'vario'
  | 'speed'
  | 'mapCenter';

export interface CursorReadoutValue {
  time: string;
  gps: string | null;
  baroAlt: string | null;
  pathSpeed: string;
  tas: string | null;
  vario: string | null;
  speed: string;
}

export interface CursorReadoutWidths {
  time: number;
  gps: number | undefined;
  baroAlt: number | undefined;
  pathSpeed: number;
  tas: number | undefined;
  vario: number | undefined;
  speed: number;
}
