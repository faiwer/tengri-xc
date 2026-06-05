import { InboxOutlined } from '@ant-design/icons';
import { Button, Modal } from 'antd';
import { useMemo, useState } from 'react';
import { peekTrack } from '../../api/tracks';
import type { TrackPeekMetadata } from '../../api/tracks.io';
import { DropZone } from '../../components/DropZone';
import { FitBounds, MapView, TrackPolyline } from '../../components/MapView';
import { LoadingIcon } from '../../components/icons/LoadingIcon';
import { useAsync, useErrorToast } from '../../core/hooks';
import type { Track } from '../../track';
import { pathsBounds, trackToPaths } from '../../track/toPaths';
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
        <UploadLoading />
      ) : preview ? (
        <UploadPreviewPanel preview={preview} />
      ) : (
        <DropZone
          extensions={SUPPORTED_EXTENSIONS}
          mode="single"
          invalidContent={
            <div>
              <InboxOutlined className={styles.invalidIcon} />
              <div className={styles.invalidText}>Unsupported file type.</div>
              <div className={styles.hint}>
                Supported formats: {SUPPORTED_EXTENSIONS.join(', ')}.
              </div>
            </div>
          }
          onDropFiles={(files) => void uploadFlight(files[0])}
        >
          <div>
            <InboxOutlined className={styles.icon} />
            <div className={styles.text}>Drop a flight file here.</div>
            <div className={styles.hint}>
              Supported formats: {SUPPORTED_EXTENSIONS.join(', ')}.
            </div>
          </div>
        </DropZone>
      )}
    </Modal>
  );
}

const SUPPORTED_EXTENSIONS = ['igc', 'gpx', 'kml', 'kmz'];

function UploadLoading() {
  return (
    <div className={styles.loading}>
      <LoadingIcon />
    </div>
  );
}

interface UploadPreview {
  metadata: TrackPeekMetadata;
  track: Track;
}

function UploadPreviewPanel({ preview }: { preview: UploadPreview }) {
  const paths = useMemo(() => trackToPaths(preview.track), [preview.track]);
  const bounds = useMemo(() => pathsBounds(paths), [paths]);

  return (
    <div>
      <div className={styles.mapSlot}>
        <MapView initialBounds={bounds} hideControls>
          <TrackPolyline paths={paths} />
          <FitBounds bounds={bounds} skipInitialFit={!!bounds} />
        </MapView>
      </div>
      <div className={styles.actions}>
        <Button type="primary" onClick={() => {}}>
          Continue
        </Button>
      </div>
    </div>
  );
}
