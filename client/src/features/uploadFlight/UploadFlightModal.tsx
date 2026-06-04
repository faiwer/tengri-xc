import { InboxOutlined } from '@ant-design/icons';
import { Modal } from 'antd';
import { DropZone } from '../../components/DropZone';
import styles from './UploadFlightModal.module.scss';

interface UploadFlightModalProps {
  open: boolean;
  onClose: () => void;
}

export function UploadFlightModal({ open, onClose }: UploadFlightModalProps) {
  return (
    <Modal title="Upload flight" open={open} onCancel={onClose} onOk={onClose}>
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
      >
        <div>
          <InboxOutlined className={styles.icon} />
          <div className={styles.text}>Drop a flight file here.</div>
          <div className={styles.hint}>
            Supported formats: {SUPPORTED_EXTENSIONS.join(', ')}.
          </div>
        </div>
      </DropZone>
    </Modal>
  );
}

const SUPPORTED_EXTENSIONS = ['igc', 'gpx', 'kml', 'kmz'];
