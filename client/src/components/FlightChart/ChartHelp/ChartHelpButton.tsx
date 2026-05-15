import { QuestionCircleOutlined } from '@ant-design/icons';
import { Tooltip } from 'antd';
import { GROUND_SPEED_WINDOW_HALF_SECONDS } from '../../../track/groundSpeed';
import { VARIO_WINDOW_HALF_SECONDS } from '../../../track/varioSegments/vario';
import { CHART_COLORS } from '../chartColors';
import { CHART_LABELS, type ChartKind } from '../types';
import styles from './ChartHelpButton.module.scss';

interface ChartHelpButtonProps {
  kind: ChartKind;
  hasBaro: boolean;
  hasTas: boolean;
}

export function ChartHelpButton({
  kind,
  hasBaro,
  hasTas,
}: ChartHelpButtonProps) {
  return (
    <Tooltip
      placement="left"
      title={<ChartHelp kind={kind} hasBaro={hasBaro} hasTas={hasTas} />}
    >
      <span
        className={styles.help}
        role="img"
        aria-label="Explain active chart lines"
      >
        <QuestionCircleOutlined />
      </span>
    </Tooltip>
  );
}

function ChartHelp({ kind, hasBaro, hasTas }: ChartHelpButtonProps) {
  const items = chartHelpItems(kind, hasBaro, hasTas);

  return (
    <div className={styles.content}>
      <div className={styles.title}>{CHART_LABELS[kind]}</div>
      <p className={styles.description}>
        {chartHelpDescription(kind, hasBaro)}
      </p>
      <ul className={styles.list}>
        {items.map(({ color, label, text }) => (
          <li key={label} className={styles.item}>
            <span className={styles.bullet} style={{ color }} />
            <span>
              <strong>{label}</strong>: {text}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

interface ChartHelpItem {
  color: string;
  label: string;
  text: string;
}

const chartHelpItems = (
  kind: ChartKind,
  hasBaro: boolean,
  hasTas: boolean,
): ChartHelpItem[] => {
  switch (kind) {
    case 'altitude':
      return hasBaro
        ? [
            {
              color: CHART_COLORS.altitudePrimary,
              label: 'Baro',
              text: 'pressure altitude from the recorder, smoothest for climb and gain',
            },
            {
              color: CHART_COLORS.altitudeOverlay,
              label: 'GPS',
              text: 'better for absolute height, noisier for altitude differences',
            },
          ]
        : [
            {
              color: CHART_COLORS.altitudePrimary,
              label: 'Altitude',
              text: 'GPS altitude over the flight window',
            },
          ];

    case 'speed':
      return [
        {
          color: CHART_COLORS.speedGps,
          label: 'GPS',
          text: 'cross-country ground speed',
        },
        {
          color: CHART_COLORS.speedPath,
          label: 'Path',
          text: 'turn-aware speed along the trace',
        },
        ...(hasTas
          ? [
              {
                color: CHART_COLORS.speedTas,
                label: 'TAS',
                text: 'recorded true airspeed',
              },
            ]
          : []),
      ];

    case 'vario':
      return [
        {
          color: CHART_COLORS.varioClimb,
          label: 'Climb',
          text: `positive vertical speed`,
        },
        {
          color: CHART_COLORS.varioSink,
          label: 'Sink',
          text: `negative vertical speed`,
        },
      ];
  }
};

const chartHelpDescription = (kind: ChartKind, hasBaro: boolean): string => {
  switch (kind) {
    case 'altitude':
      return hasBaro
        ? 'Shows barometric and GPS altitude over the flight window.'
        : 'Shows GPS altitude over the flight window.';
    case 'speed':
      return `Speed lines use centred ${formatWindowSeconds(
        GROUND_SPEED_WINDOW_HALF_SECONDS,
      )} windows.`;
    case 'vario':
      const varioWindow = formatWindowSeconds(VARIO_WINDOW_HALF_SECONDS);
      return hasBaro
        ? `Vertical speed is smoothed over a centred ${varioWindow} window using barometric altitude.`
        : `Vertical speed is smoothed over a centred ${varioWindow} window using GPS altitude.`;
  }
};

const formatWindowSeconds = (halfSeconds: number): string =>
  `${halfSeconds * 2}-second`;
