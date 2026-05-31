import { Dropdown, Button } from 'antd';
import type { MenuProps } from 'antd';
import styles from './MapView.module.scss';
import { type MapType } from './types';

interface MapTypeSwitcherProps {
  mapType: MapType;
  setMapType: (mapType: MapType) => void;
}

export function MapTypeSwitcher({ mapType, setMapType }: MapTypeSwitcherProps) {
  return (
    <Dropdown
      menu={{
        items: MAP_TYPE_ITEMS,
        onClick: ({ key }) => setMapType(key as MapType),
      }}
      trigger={['click']}
    >
      <Button className={styles.mapTypeSwitcher}>{mapType}</Button>
    </Dropdown>
  );
}

const MAP_TYPE_ITEMS: MenuProps['items'] = [
  { key: 'roadmap', label: 'Roadmap' },
  { key: 'terrain', label: 'Terrain' },
  { key: 'satellite', label: 'Satellite' },
  { key: 'hybrid', label: 'Hybrid' },
];
