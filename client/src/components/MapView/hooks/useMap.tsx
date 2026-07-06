import { nullthrows } from '../../../utils/nullthrows';
import { useMap as useMapLibre } from '@vis.gl/react-maplibre';

export function useMap() {
  return nullthrows(useMapLibre().current?.getMap());
}
