import { describe, expect, it, vi } from 'vitest';
import type { ChartHelpItem } from '../../components/FlightChart/ChartHelp';
import { buildFields } from './buildFields';
import type { CursorReadoutValue, CursorReadoutWidths } from './types';

vi.mock('./fields', () => ({
  field: (
    key: string,
    tooltip: unknown,
    value: string,
    width?: number,
    color?: string,
  ) => ({
    key,
    tooltip,
    value,
    width,
    color,
    icon: null,
  }),
}));

describe('buildFields', () => {
  it('omits altitude and vario fields when altitude data is missing', () => {
    const fields = buildFields(
      'altitude',
      {
        time: '2:10:00',
        gps: null,
        baroAlt: null,
        pathSpeed: '12 km/h',
        tas: null,
        vario: null,
        speed: '10 km/h',
      },
      {
        time: 7,
        gps: undefined,
        baroAlt: undefined,
        pathSpeed: 7,
        tas: undefined,
        vario: undefined,
        speed: 7,
      },
      HELP_ALTITUDE,
    );

    expect(fields.map((field) => field.key)).toEqual(['time', 'speed']);
  });

  it('keeps altitude and vario fields when altitude data is present', () => {
    const readout: CursorReadoutValue = {
      time: '2:10:00',
      gps: '1,200 m',
      baroAlt: null,
      pathSpeed: '12 km/h',
      tas: null,
      vario: '1.2 m/s',
      speed: '10 km/h',
    };
    const widths: CursorReadoutWidths = {
      time: 7,
      gps: 7,
      baroAlt: undefined,
      pathSpeed: 7,
      tas: undefined,
      vario: 7,
      speed: 7,
    };

    const fields = buildFields('speed', readout, widths, HELP_SPEED);

    expect(fields.map((field) => field.key)).toEqual([
      'time',
      'gps',
      'vario',
      'speed',
      'pathSpeed',
    ]);
  });
});

const HELP_ALTITUDE: ChartHelpItem[] = [
  {
    kind: 'altitude',
    color: 'blue',
    label: 'Altitude',
    text: 'GPS altitude',
  },
];

const HELP_SPEED: ChartHelpItem[] = [
  {
    kind: 'gps',
    color: 'blue',
    label: 'GPS',
    text: 'ground speed',
  },
  {
    kind: 'path',
    color: 'orange',
    label: 'Path',
    text: 'path speed',
  },
];
