import { apiGet } from './core';
import { TrackMetadataIo, type TrackMetadata } from './tracks.io';

export async function getTrackMetadata(
  trackId: string,
): Promise<TrackMetadata> {
  return apiGet(`/tracks/${trackId}/md`, TrackMetadataIo);
}
