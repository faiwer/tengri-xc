import { InboxOutlined } from '@ant-design/icons';
import { DropZone } from '../../components/DropZone';
import styles from './UploadDropZone.module.scss';

export function UploadDropZone({ onFile }: { onFile: (file: File) => void }) {
  return (
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
      onDropFiles={(files) => onFile(files[0])}
    >
      <div>
        <InboxOutlined className={styles.icon} />
        <div className={styles.text}>Drop a flight file here.</div>
        <div className={styles.hint}>
          Supported formats: {SUPPORTED_EXTENSIONS.join(', ')}.
        </div>
      </div>
    </DropZone>
  );
}

const SUPPORTED_EXTENSIONS = ['igc', 'gpx', 'kml', 'kmz'];
