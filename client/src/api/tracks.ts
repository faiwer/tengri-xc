import { decode as bincodeDecode } from 'bincode-ts';
import { decodeTrack, type DecodeOptions, type Track } from '../track';
import { apiGet, apiGetBlob } from './core';
import { TengriFileIo, TrackMetadataIo, type TrackMetadata } from './tracks.io';

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
 * Fetch the bincode-encoded TengriFile and reconstruct the in-memory `Track`.
 * Consumers should never see the wire-level CompactTrack.
 *
 * Decoding is sliced across animation frames; pass `decode.signal` to cancel
 * if the caller (e.g. a route effect) is torn down mid-flight.
 */
export async function getTrack(
  trackId: string,
  kind: TrackKind = 'full',
  decode: DecodeOptions = {},
): Promise<Track> {
  const blob = await getTrackBlob(trackId, kind);
  const buffer = await blob.arrayBuffer();
  const { value } = bincodeDecode(TengriFileIo, buffer);
  return decodeTrack(value, decode);
}
