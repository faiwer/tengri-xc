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
  gps: string;
  baroAlt: string | null;
  pathSpeed: string;
  tas: string | null;
  vario: string;
  speed: string;
}

export interface CursorReadoutWidths {
  time: number;
  gps: number;
  baroAlt: number | undefined;
  pathSpeed: number;
  tas: number | undefined;
  vario: number;
  speed: number;
}
