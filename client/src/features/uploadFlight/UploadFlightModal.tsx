import { Modal } from 'antd';
import { useState } from 'react';
import type { RecentGlider } from '../../api/me/recentGliders.io';
import { peekTrack } from '../../api/tracks';
import { LoadingIcon } from '../../components/icons/LoadingIcon';
import { useAsync, useErrorToast } from '../../core/hooks';
import { nullthrows } from '../../utils/nullthrows';
import { FlightDetailsStep } from './FlightDetailsStep';
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
  const [step, setStep] = useState<Step>('source');
  const [preview, setPreview] = useState<UploadPreview | null>(null);
  const [glider, setGlider] = useState<RecentGlider | null>(null);

  const [uploadFlight, isUploading, uploadError] = useAsync(
    async (file: File) => {
      setPreview(await peekTrack(file));
      setStep('preview');
    },
  );
  useErrorToast(uploadError, { title: "Couldn't preview flight" });

  return (
    <Modal
      title={STEP_TITLES[step] ?? 'Upload flight'}
      open={open}
      footer={null}
      width={760}
      onCancel={() => {
        setStep('source');
        setPreview(null);
        setGlider(null);
        onClose();
      }}
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
        <FlightDetailsStep preview={nullthrows(preview)} glider={glider} />
      )}
    </Modal>
  );
}

const STEP_TITLES: Partial<Record<Step, string>> = {
  glider: 'Copy data from previous flights?',
};
