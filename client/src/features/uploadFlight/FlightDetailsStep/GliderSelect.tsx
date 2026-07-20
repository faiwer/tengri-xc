import { Select } from 'antd';
import { useMemo } from 'react';
import type { GliderCatalog } from '../../../api/admin/gliders.io';
import styles from './FlightDetailsStep.module.scss';

interface GliderSelectProps {
  catalog: GliderCatalog | null;
  isLoading: boolean;
  brandId: string | null;
  modelId: string | null;
  onBrandChange: (brandId: string) => void;
  onModelChange: (modelId: string) => void;
}

export function GliderSelect({
  catalog,
  isLoading,
  brandId,
  modelId,
  onBrandChange,
  onModelChange,
}: GliderSelectProps) {
  const brandOptions = useMemo(
    () =>
      catalog?.brands.map((brand) => ({
        value: brand.id,
        label: brand.name,
      })) ?? [],
    [catalog],
  );

  const modelOptions = useMemo(
    () =>
      catalog && brandId
        ? catalog.models
            .filter((model) => model.brandId === brandId)
            .map((model) => ({ value: model.id, label: model.name }))
        : [],
    [catalog, brandId],
  );

  const ready = catalog != null && !isLoading;

  return (
    <div className={styles.gliderRow}>
      <Select
        className={styles.select}
        showSearch={{ optionFilterProp: 'label' }}
        placeholder="Brand"
        disabled={!ready}
        loading={isLoading}
        value={brandId ?? undefined}
        options={brandOptions}
        onChange={onBrandChange}
      />
      <Select
        className={styles.select}
        showSearch={{ optionFilterProp: 'label' }}
        placeholder="Model"
        disabled={!ready || brandId == null}
        value={modelId ?? undefined}
        options={modelOptions}
        onChange={onModelChange}
      />
    </div>
  );
}
