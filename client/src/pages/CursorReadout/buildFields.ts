import type { ChartKind } from '../../components/FlightChart';
import type { ChartHelpItem } from '../../components/FlightChart/ChartHelp';
import { keyByField } from '../../utils/keyBy';
import { field } from './fields';
import type { buildCursorReadout, buildCursorReadoutWidths } from './readout';
import type { CursorReadoutField } from './types';

export const buildFields = (
  activeChartKind: ChartKind,
  readout: ReturnType<typeof buildCursorReadout>,
  widths: ReturnType<typeof buildCursorReadoutWidths>,
  helpItems: ChartHelpItem[],
): CursorReadoutField[] => {
  const help = keyByField(helpItems, 'kind');

  const fields: {
    time: CursorReadoutField;
    altitude: CursorReadoutField[];
    vario: CursorReadoutField[];
    speed: CursorReadoutField | CursorReadoutField[];
  } = {
    time: field('time', 'Time', readout.time, widths.time),
    altitude: readout.gps
      ? [field('gps', 'GPS altitude', readout.gps, widths.gps)]
      : [],
    vario: readout.vario
      ? [field('vario', 'Vertical speed', readout.vario, widths.vario)]
      : [],
    speed: field('speed', 'Ground speed', readout.speed, widths.speed),
  };

  switch (activeChartKind) {
    case 'altitude':
      if (readout.gps) {
        const primary = help[readout.baroAlt ? 'gps' : 'altitude'];
        fields.altitude = [
          field('gps', primary, readout.gps, widths.gps, iconColorOf(primary)),
        ];

        if (readout.baroAlt) {
          fields.altitude.push(
            field(
              'baroAlt',
              help.baro,
              readout.baroAlt,
              widths.baroAlt,
              iconColorOf(help.baro),
            ),
          );
        }
      }

      if (readout.ground) {
        fields.altitude.push(
          field(
            'ground',
            help.ground,
            readout.ground,
            widths.ground,
            iconColorOf(help.ground),
          ),
        );
      }
      break;

    case 'speed':
      fields.speed = [
        field(
          'speed',
          help.gps,
          readout.speed,
          widths.speed,
          iconColorOf(help.gps),
        ),
        field(
          'pathSpeed',
          help.path,
          readout.pathSpeed,
          widths.pathSpeed,
          iconColorOf(help.path),
        ),
      ];
      break;
  }

  return [
    fields.time,
    ...fields.altitude,
    ...fields.vario,
    ...(Array.isArray(fields.speed) ? fields.speed : [fields.speed]),
  ];
};

/** Readout icon colour: {@link ChartHelpItem.iconColor} when set, else `color`. */
const iconColorOf = (item: ChartHelpItem): string =>
  item.iconColor ?? item.color;
