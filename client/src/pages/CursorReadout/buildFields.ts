import { chartHelpItems } from '../../components/FlightChart/ChartHelp';
import type { ChartKind } from '../../components/FlightChart';
import { keyByField } from '../../utils/keyBy';
import { field } from './fields';
import type { buildCursorReadout, buildCursorReadoutWidths } from './readout';
import type { CursorReadoutField } from './types';

export const buildFields = (
  activeChartKind: ChartKind,
  readout: ReturnType<typeof buildCursorReadout>,
  widths: ReturnType<typeof buildCursorReadoutWidths>,
  helpItems: ReturnType<typeof chartHelpItems>,
): CursorReadoutField[] => {
  const help = keyByField(helpItems, 'kind');

  const fields: {
    time: CursorReadoutField;
    altitude: CursorReadoutField | CursorReadoutField[];
    vario: CursorReadoutField | CursorReadoutField[];
    speed: CursorReadoutField | CursorReadoutField[];
  } = {
    time: field('time', 'Time', readout.time, widths.time),
    altitude: field('gps', 'GPS altitude', readout.gps, widths.gps),
    vario: field('vario', 'Vertical speed', readout.vario, widths.vario),
    speed: field('speed', 'Ground speed', readout.speed, widths.speed),
  };

  switch (activeChartKind) {
    case 'altitude':
      fields.altitude = [
        field(
          'gps',
          help[readout.baroAlt ? 'gps' : 'altitude'],
          readout.gps,
          widths.gps,
          help[readout.baroAlt ? 'gps' : 'altitude'].color,
        ),
      ];

      if (readout.baroAlt) {
        fields.altitude.push(
          field(
            'baroAlt',
            help.baro,
            readout.baroAlt,
            widths.baroAlt,
            help.baro.color,
          ),
        );
      }
      break;

    case 'speed':
      fields.speed = [
        field('speed', help.gps, readout.speed, widths.speed, help.gps.color),
        field(
          'pathSpeed',
          help.path,
          readout.pathSpeed,
          widths.pathSpeed,
          help.path.color,
        ),
      ];
      break;
  }

  return [
    fields.time,
    ...(Array.isArray(fields.altitude) ? fields.altitude : [fields.altitude]),
    ...(Array.isArray(fields.vario) ? fields.vario : [fields.vario]),
    ...(Array.isArray(fields.speed) ? fields.speed : [fields.speed]),
  ];
};
