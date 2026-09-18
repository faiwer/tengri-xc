import { EyeOutlined } from '@ant-design/icons';
import { Tooltip } from 'antd';
import clsx from 'clsx';
import { useState } from 'react';
import { getTrackPreviewImageUrl } from '../../api/tracks';
import { LoadingIcon } from '../../components/icons/LoadingIcon';
import styles from './FlightPreview.module.scss';

/** EyeOutlined that reveals the flight's rendered picture on hover. */
export function FlightPreview({ flightId }: { flightId: string }) {
  return (
    <Tooltip
      // The column is the table's right edge, so the picture grows inwards.
      placement="bottomRight"
      title={<PreviewImage flightId={flightId} />}
      styles={{ container: { padding: 0 } }}
    >
      <EyeOutlined className={styles.trigger} />
    </Tooltip>
  );
}

/**
 * The server renders the picture on the first request for it, so the wait can
 * run into seconds — long enough that the spinner is the point, not a formality.
 */
function PreviewImage({ flightId }: { flightId: string }) {
  const [loaded, setLoaded] = useState(false);

  return (
    <>
      {!loaded && <LoadingIcon className={styles.loading} inverseTheme />}
      {/* Kept mounted while hidden: `display: none` still fetches. */}
      <img
        className={clsx(styles.image, !loaded && styles.pending)}
        src={getTrackPreviewImageUrl(flightId)}
        alt=""
        onLoad={() => setLoaded(true)}
      />
    </>
  );
}
