import { decode } from 'bincode-ts';
import { apiGet, apiGetBlob } from './core';
import {
  TengriFileIo,
  TrackMetadataIo,
  type TengriFile,
  type TrackMetadata,
} from './tracks.io';

export async function getTrackMetadata(
  trackId: string,
): Promise<TrackMetadata> {
  return apiGet(`/tracks/${trackId}/md`, TrackMetadataIo);
}

export type TrackKind = 'full' | 'preview';

/**
 * Fetch the raw track Blob. The server sets `Content-Encoding: gzip`, so
 * `fetch` already decompresses it: the Blob holds plain bincode bytes.
 */
export async function getTrackBlob(
  trackId: string,
  kind: TrackKind = 'full',
): Promise<Blob> {
  return apiGetBlob(`/tracks/${kind}/${trackId}`);
}

/**
 * Fetch and decode the bincode-encoded TengriFile for the given track.
 */
export async function getTrack(
  trackId: string,
  kind: TrackKind = 'full',
): Promise<TengriFile> {
  const blob = await getTrackBlob(trackId, kind);
  const buffer = await blob.arrayBuffer();
  const { value } = decode(TengriFileIo, buffer);
  return value;
}
