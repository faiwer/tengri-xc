import { decode as bincodeDecode } from 'bincode-ts';
import { decodeTrack, type DecodeOptions, type Track } from '../track';
import { apiGet, apiGetBlob, apiPostRaw, type ApiRequestOptions } from './core';
import {
  TengriFileIo,
  TrackMetadataIo,
  TrackPeekResponseIo,
  TracksPageIo,
  type TrackMetadata,
  type TrackPeekMetadata,
  type TracksPage,
} from './tracks.io';

export async function getTrackMetadata(
  trackId: string,
): Promise<TrackMetadata> {
  return apiGet(`/tracks/${trackId}/md`, TrackMetadataIo);
}

export interface GetTracksPageParams {
  /** Pass through the `nextCursor` from the previous page. */
  cursor?: string;
  /** Server caps at 100; defaults to 25 when omitted. */
  limit?: number;
  /** Cancel the in-flight request — see {@link apiGet}. */
  signal?: AbortSignal;
}

/**
 * Fetch one cursor-paginated page of the global tracks feed
 * (`GET /tracks`).
 */
export async function getTracksPage(
  params: GetTracksPageParams = {},
): Promise<TracksPage> {
  const query = new URLSearchParams();
  if (params.cursor) {
    query.set('cursor', params.cursor);
  }

  if (params.limit !== undefined) {
    query.set('limit', params.limit.toString());
  }

  const suffix = query.size > 0 ? `?${query}` : '';
  return apiGet(`/tracks${suffix}`, TracksPageIo, {
    signal: params.signal,
  });
}

export interface PeekTrackResult {
  track: Track;
  metadata: TrackPeekMetadata;
}

export async function peekTrack(
  file: File,
  options: ApiRequestOptions = {},
): Promise<PeekTrackResult> {
  const form = new FormData();
  form.append('flight', await getPreviewFileGZipBlob(file), file.name);
  const response = await apiPostRaw(
    '/tracks/peek',
    form,
    TrackPeekResponseIo,
    options,
  );
  const flight = base64ToBlob(response.flight);
  return {
    track: await decodeTrackBlob(flight),
    metadata: response.metadata,
  };
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

export async function decodeTrackBlob(
  blob: Blob,
  decode: DecodeOptions = {},
): Promise<Track> {
  const buffer = await blob.arrayBuffer();
  const { value } = bincodeDecode(TengriFileIo, buffer);
  return decodeTrack(value, decode);
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
  return decodeTrackBlob(blob, decode);
}

async function getPreviewFileGZipBlob(file: File): Promise<Blob | File> {
  if (extractFileExtension(file) === 'kmz') {
    return file; // It's already a ZIP-wrapper over KML.
  }

  const compressed = file.stream().pipeThrough(new CompressionStream('gzip'));
  // `Response` is just a local Web Streams collector here; no request is made.
  return new Response(compressed).blob();
}

const extractFileExtension = (file: File): string =>
  file.name.split('.').pop()?.toLowerCase() ?? '';

function base64ToBlob(base64: string): Blob {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return new Blob([bytes]);
}
