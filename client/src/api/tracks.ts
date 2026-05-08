import { decode as bincodeDecode } from 'bincode-ts';
import { decodeTrack, type DecodeOptions, type Track } from '../track';
import { apiGet, apiGetBlob } from './core';
import {
  TengriFileIo,
  TrackMetadataIo,
  TracksPageIo,
  type TrackListItem,
  type TrackMetadata,
} from './tracks.io';

export async function getTrackMetadata(
  trackId: string,
): Promise<TrackMetadata> {
  return apiGet(`/tracks/${trackId}/md`, TrackMetadataIo);
}

export interface TracksPageResult {
  items: TrackListItem[];
  /** Opaque cursor for the next page; `null` on the last page. */
  nextCursor: string | null;
}

export interface GetTracksPageParams {
  /** Pass through the `next_cursor` from the previous page. */
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
): Promise<TracksPageResult> {
  const query = new URLSearchParams();
  if (params.cursor) {
    query.set('cursor', params.cursor);
  }

  if (params.limit !== undefined) {
    query.set('limit', params.limit.toString());
  }

  const suffix = query.size > 0 ? `?${query}` : '';
  const page = await apiGet(`/tracks${suffix}`, TracksPageIo, {
    signal: params.signal,
  });

  return { items: page.items, nextCursor: page.next_cursor };
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
