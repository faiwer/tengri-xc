import { Button } from 'antd';
import { useMemo } from 'react';
import type { TrackPeekMetadata } from '../../api/tracks.io';
import { FitBounds, MapView, TrackPolyline } from '../../components/MapView';
import type { Track } from '../../track';
import { pathsBounds, trackToPaths } from '../../track/toPaths';
import { RoutesSummary } from './RoutesSummary';
import styles from './UploadPreviewPanel.module.scss';

export interface UploadPreview {
  metadata: TrackPeekMetadata;
  track: Track;
}

export function UploadPreviewPanel({
  preview,
  onContinue,
}: {
  preview: UploadPreview;
  onContinue: () => void;
}) {
  const paths = useMemo(() => trackToPaths(preview.track), [preview.track]);
  const bounds = useMemo(() => pathsBounds(paths), [paths]);

  return (
    <div>
      <div className={styles.mapSlot}>
        <MapView
          initialBounds={bounds}
          initialPadding={MAP_PADDING_PX}
          hideControls
        >
          <TrackPolyline paths={paths} />
          <FitBounds
            bounds={bounds}
            skipInitialFit={!!bounds}
            padding={MAP_PADDING_PX}
          />
        </MapView>
      </div>
      <div className={styles.actions}>
        <RoutesSummary metadata={preview.metadata} />
        <Button type="primary" onClick={onContinue}>
          Continue
        </Button>
      </div>
    </div>
  );
}

const MAP_PADDING_PX = 30;
