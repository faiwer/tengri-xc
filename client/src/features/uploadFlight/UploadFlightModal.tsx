import { Modal } from 'antd';
import { useState } from 'react';
import { peekTrack } from '../../api/tracks';
import { LoadingIcon } from '../../components/icons/LoadingIcon';
import { useAsync, useErrorToast } from '../../core/hooks';
import { UploadDropZone } from './UploadDropZone';
import { UploadPreviewPanel, type UploadPreview } from './UploadPreviewPanel';
import styles from './UploadFlightModal.module.scss';

interface UploadFlightModalProps {
  open: boolean;
  onClose: () => void;
}

export function UploadFlightModal({ open, onClose }: UploadFlightModalProps) {
  const [preview, setPreview] = useState<UploadPreview | null>(null);
  const [uploadFlight, isUploading, uploadError] = useAsync(
    async (file: File) => {
      setPreview(null);
      setPreview(await peekTrack(file));
    },
  );
  useErrorToast(uploadError, { title: "Couldn't preview flight" });

  return (
    <Modal
      title="Upload flight"
      open={open}
      footer={null}
      width={760}
      onCancel={() => {
        setPreview(null);
        onClose();
      }}
    >
      {isUploading ? (
        <div className={styles.loading}>
          <LoadingIcon />
        </div>
      ) : preview ? (
        <UploadPreviewPanel preview={preview} />
      ) : (
        <UploadDropZone onFile={(file) => void uploadFlight(file)} />
      )}
    </Modal>
  );
}
