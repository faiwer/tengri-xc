import { Modal } from 'antd';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { createFlight } from '../../api/me/flights';
import type { RecentGlider } from '../../api/me/recentGliders.io';
import { peekTrack } from '../../api/tracks';
import { LoadingIcon } from '../../components/icons/LoadingIcon';
import { useAsync, useErrorToast } from '../../core/hooks';
import { routes } from '../../core/routes';
import { isCatalogSport } from '../../core/sport';
import { nullthrows } from '../../utils/nullthrows';
import {
  FlightDetailsStep,
  type FlightDetails,
  type FlightDetailsForm,
} from '../flightDetails';
import { GliderPickerStep } from './GliderPickerStep';
import { UploadDropZone } from './UploadDropZone';
import { UploadPreviewPanel, type UploadPreview } from './UploadPreviewPanel';
import styles from './UploadFlightModal.module.scss';

interface UploadFlightModalProps {
  open: boolean;
  onClose: () => void;
}

type Step = 'source' | 'preview' | 'glider' | 'details';

export function UploadFlightModal({ open, onClose }: UploadFlightModalProps) {
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>('source');
  const [file, setFile] = useState<File | null>(null);
  const [preview, setPreview] = useState<UploadPreview | null>(null);
  const [glider, setGlider] = useState<RecentGlider | null>(null);

  const [uploadFlight, isUploading, uploadError] = useAsync(
    async (picked: File) => {
      setFile(picked);
      setPreview(await peekTrack(picked));
      setStep('preview');
    },
  );
  useErrorToast(uploadError, { title: "Couldn't preview flight" });

  const [submitFlight, isSubmitting, submitError] = useAsync(
    async (details: FlightDetails) => {
      const { id } = await createFlight(nullthrows(file), details);
      close();
      navigate(routes.flight(id));
    },
  );
  useErrorToast(submitError, { title: "Couldn't upload flight" });

  const close = () => {
    setStep('source');
    setFile(null);
    setPreview(null);
    setGlider(null);
    onClose();
  };

  return (
    <Modal
      title={STEP_TITLES[step] ?? 'Upload flight'}
      open={open}
      footer={null}
      width={760}
      maskClosable={!isSubmitting}
      onCancel={isSubmitting ? undefined : close}
      className={styles.modal}
    >
      {step === 'source' ? (
        isUploading ? (
          <div className={styles.loading}>
            <LoadingIcon />
          </div>
        ) : (
          <UploadDropZone onFile={(file) => void uploadFlight(file)} />
        )
      ) : step === 'preview' ? (
        <UploadPreviewPanel
          preview={nullthrows(preview)}
          onContinue={() => setStep('glider')}
        />
      ) : step === 'glider' ? (
        <GliderPickerStep
          onSelect={(picked) => {
            setGlider(picked);
            setStep('details');
          }}
        />
      ) : (
        <FlightDetailsStep
          initial={initialForm(glider)}
          isSubmitting={isSubmitting}
          onCancel={close}
          onSubmit={(details) => void submitFlight(details)}
        />
      )}
    </Modal>
  );
}

const STEP_TITLES: Partial<Record<Step, string>> = {
  glider: 'Copy data from previous flights?',
};

/** Seed the details form from a copied recent glider, or an empty draft. */
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
