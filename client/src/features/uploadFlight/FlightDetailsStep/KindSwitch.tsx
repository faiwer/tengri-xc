import { Segmented, Tooltip } from 'antd';
import type { Sport } from '../../../api/admin/gliders.io';
import { GliderKindIcon } from '../../../components/icons/GliderKindIcon';
import { CATALOG_SPORTS, type CatalogSport } from '../../../core/sport';
import styles from './FlightDetailsStep.module.scss';

export function KindSwitch({
  value,
  onChange,
}: {
  value: Sport;
  onChange: (kind: Sport) => void;
}) {
  return (
    <Segmented<Sport>
      className={styles.kindSwitch}
      value={value}
      onChange={onChange}
      options={CATALOG_SPORTS.map((kind) => ({
        value: kind,
        label: (
          <Tooltip title={KIND_TOOLTIPS[kind]}>
            <span className={styles.kindOption}>
              <GliderKindIcon kind={kind} aria-label={KIND_TOOLTIPS[kind]} />
            </span>
          </Tooltip>
        ),
      }))}
    />
  );
}

const KIND_TOOLTIPS: Record<CatalogSport, string> = {
  hg: 'Hang gliding',
  pg: 'Paragliding',
  sp: 'Sailplane',
};
