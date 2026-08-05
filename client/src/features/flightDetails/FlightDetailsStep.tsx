import { Button } from 'antd';
import { type ReactNode, useState } from 'react';
import type { Sport } from '../../api/admin/gliders.io';
import type { LaunchMethod, Propulsion } from '../../api/flights.io';
import { nullthrows } from '../../utils/nullthrows';
import { GliderSelect } from './GliderSelect';
import { KindSwitch } from './KindSwitch';
import { LaunchMethodSelect } from './LaunchMethodSelect';
import { PropulsionSelect } from './PropulsionSelect';
import { useGliderCatalog } from './useGliderCatalog';
import type { FlightDetails, FlightDetailsForm } from './types';
import styles from './FlightDetailsStep.module.scss';

interface FlightDetailsStepProps {
  /** Seed values; the working state is a copy, so edits don't mutate the caller. */
  initial: FlightDetailsForm;
  /** `true` while the submit request is in flight — locks the form. */
  isSubmitting: boolean;
  /** Primary button label. Defaults to `Submit`. */
  submitLabel?: string;
  onSubmit: (value: FlightDetails) => void;
  onCancel: () => void;
}

export function FlightDetailsStep({
  initial,
  isSubmitting,
  submitLabel = 'Submit',
  onSubmit,
  onCancel,
}: FlightDetailsStepProps) {
  const [form, setForm] = useState<FlightDetailsForm>(initial);
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
        <KindSwitch
          value={form.kind}
          onChange={onKindChange}
          disabled={isSubmitting}
        />
      </Field>
      <Field label="Glider">
        <GliderSelect
          catalog={catalog}
          isLoading={isLoading}
          brandId={form.brandId}
          modelId={form.modelId}
          onBrandChange={onBrandChange}
          onModelChange={onModelChange}
          disabled={isSubmitting}
        />
      </Field>
      <Field label="Launch">
        <LaunchMethodSelect
          value={form.launchMethod}
          onChange={onLaunchChange}
          disabled={isSubmitting}
        />
      </Field>
      <Field label="Propulsion">
        <PropulsionSelect
          value={form.propulsion}
          onChange={onPropulsionChange}
          disabled={isSubmitting}
        />
      </Field>
      <div className={styles.actions}>
        <Button onClick={onCancel} disabled={isSubmitting}>
          Cancel
        </Button>
        <Button
          type="primary"
          disabled={!isComplete}
          loading={isSubmitting}
          onClick={onSubmitClick}
        >
          {submitLabel}
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
