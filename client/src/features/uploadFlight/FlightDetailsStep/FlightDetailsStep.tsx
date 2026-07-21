import { Button } from 'antd';
import { type ReactNode, useState } from 'react';
import type { Sport } from '../../../api/admin/gliders.io';
import type { LaunchMethod, Propulsion } from '../../../api/flights.io';
import { isCatalogSport } from '../../../core/sport';
import type { RecentGlider } from '../../../api/me/recentGliders.io';
import { nullthrows } from '../../../utils/nullthrows';
import type { UploadPreview } from '../UploadPreviewPanel';
import { GliderSelect } from './GliderSelect';
import { KindSwitch } from './KindSwitch';
import { LaunchMethodSelect } from './LaunchMethodSelect';
import { PropulsionSelect } from './PropulsionSelect';
import { useGliderCatalog } from './useGliderCatalog';
import type { FlightDetails, FlightDetailsForm } from './types';
import styles from './FlightDetailsStep.module.scss';

interface FlightDetailsStepProps {
  preview: UploadPreview;
  /** Glider picked in the previous step, or `null` when skipped. */
  glider: RecentGlider | null;
  onSubmit: (value: FlightDetails) => void;
  onCancel: () => void;
}

export function FlightDetailsStep({
  glider,
  onSubmit,
  onCancel,
}: FlightDetailsStepProps) {
  const [form, setForm] = useState<FlightDetailsForm>(() =>
    initialForm(glider),
  );
  const { catalog, isLoading } = useGliderCatalog(form.kind);

  const onKindChange = (kind: Sport) =>
    setForm((prev) => ({ ...prev, kind, brandId: null, modelId: null }));
  const onBrandChange = (brandId: string) =>
    setForm((prev) => ({ ...prev, brandId, modelId: null }));
  const onModelChange = (modelId: string) =>
    setForm((prev) => ({ ...prev, modelId }));
  const onLaunchChange = (launchMethod: LaunchMethod) =>
    setForm((prev) => ({ ...prev, launchMethod }));
  const onPropulsionChange = (propulsion: Propulsion) =>
    setForm((prev) => ({ ...prev, propulsion }));

  const isComplete =
    form.brandId != null &&
    form.modelId != null &&
    form.launchMethod != null &&
    form.propulsion != null;

  const onSubmitClick = () =>
    onSubmit({
      kind: form.kind,
      brandId: nullthrows(form.brandId),
      modelId: nullthrows(form.modelId),
      launchMethod: nullthrows(form.launchMethod),
      propulsion: nullthrows(form.propulsion),
    });

  return (
    <div className={styles.form}>
      <Field label="Discipline">
        <KindSwitch value={form.kind} onChange={onKindChange} />
      </Field>
      <Field label="Glider">
        <GliderSelect
          catalog={catalog}
          isLoading={isLoading}
          brandId={form.brandId}
          modelId={form.modelId}
          onBrandChange={onBrandChange}
          onModelChange={onModelChange}
        />
      </Field>
      <Field label="Launch">
        <LaunchMethodSelect
          value={form.launchMethod}
          onChange={onLaunchChange}
        />
      </Field>
      <Field label="Propulsion">
        <PropulsionSelect
          value={form.propulsion}
          onChange={onPropulsionChange}
        />
      </Field>
      <div className={styles.actions}>
        <Button onClick={onCancel}>Cancel</Button>
        <Button type="primary" disabled={!isComplete} onClick={onSubmitClick}>
          Submit
        </Button>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.field}>
      <span className={styles.label}>{label}</span>
      {children}
    </div>
  );
}

function initialForm(glider: RecentGlider | null): FlightDetailsForm {
  if (!glider) {
    return {
      kind: 'hg',
      brandId: null,
      modelId: null,
      launchMethod: null,
      propulsion: 'free',
    };
  }

  const kind = isCatalogSport(glider.kind) ? glider.kind : 'hg';
  // Only carry the brand/model over when they belong to the resolved kind's
  // catalog (they won't if we fell back off an `'other'` glider).
  const sameKind = kind === glider.kind;
  return {
    kind,
    brandId: sameKind ? glider.brandId : null,
    modelId: sameKind ? glider.modelId : null,
    launchMethod: glider.launchMethod,
    propulsion: 'free',
  };
}
