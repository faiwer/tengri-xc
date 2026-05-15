import { useLocalStorageValue } from '../../utils/useLocalStorageValue';
import { ACTIVE_KIND_STORAGE_OPTIONS } from './types';

export const useChartKind = () =>
  useLocalStorageValue('flight-chart-tab', ACTIVE_KIND_STORAGE_OPTIONS);
