import type { RecentGlider } from '../../api/me/recentGliders.io';
import type { UploadPreview } from './UploadPreviewPanel';

interface FlightDetailsStepProps {
  preview: UploadPreview;
  /** Glider picked in the previous step, or `null` when skipped. */
  glider: RecentGlider | null;
}

export function FlightDetailsStep(_props: FlightDetailsStepProps) {
  // TODO: real flight-details form (title, comments, glider/site pickers…),
  // seeded from `glider` when present.
  return <div>TODO: flight details form</div>;
}
