import { z } from 'zod';
import type { Sport } from '../admin/gliders.io';
import {
  apiDelete,
  apiPatch,
  apiPostRaw,
  apiPostVoid,
  type ApiRequestOptions,
} from '../core';
import type { LaunchMethod, Propulsion } from '../flights.io';
import { gzipFlightFile } from '../tracks';
import { TrackMetadataIo, type TrackMetadata } from '../tracks.io';

export interface CreateFlightMeta {
  kind: Sport;
  brandId: string;
  modelId: string;
  launchMethod: LaunchMethod;
  propulsion: Propulsion;
}

export const CreateFlightResponseIo = z.object({
  id: z.string(),
  /** `null` once scoring finished in-request; otherwise the queue position. */
  position: z.number().int().nullable(),
});
export type CreateFlightResponse = z.infer<typeof CreateFlightResponseIo>;

/**
 * Upload a flight: persists it and enqueues route scoring. Sends the gzipped
 * flight bytes plus the glider/launch metadata as multipart, mirroring
 * `peekTrack`. Resolves once the server responds (which is as soon as scoring
 * finishes, or after its wait ceiling).
 */
export async function createFlight(
  file: File,
  meta: CreateFlightMeta,
  options: ApiRequestOptions = {},
): Promise<CreateFlightResponse> {
  const form = new FormData();
  form.append('flight', await gzipFlightFile(file), file.name);
  form.append('kind', meta.kind);
  form.append('brand_id', meta.brandId);
  form.append('model_id', meta.modelId);
  form.append('launch_method', meta.launchMethod);
  form.append('propulsion', meta.propulsion);
  return apiPostRaw('/me/flights', form, CreateFlightResponseIo, options);
}

/** Glider/launch metadata an owner (or admin) can change after upload. */
export type UpdateFlightMeta = CreateFlightMeta;

/**
 * Edit a flight's glider/launch metadata. Allowed for the flight's owner or an
 * admin with `MANAGE_TRACKS`. The track is untouched, so route scores stay put;
 * the server returns the refreshed metadata so the page can update in place.
 */
export const updateFlight = (
  id: string,
  meta: UpdateFlightMeta,
  options: ApiRequestOptions = {},
): Promise<TrackMetadata> =>
  apiPatch(`/me/flights/${id}`, meta, TrackMetadataIo, options);

/**
 * Delete a flight and all its data. Allowed for the flight's owner or an admin
 * with `MANAGE_TRACKS`; the server cascades tracks/routes/scoring and reaps a
 * now-orphaned private glider.
 */
export const deleteFlight = (
  id: string,
  options: ApiRequestOptions = {},
): Promise<void> => apiDelete(`/me/flights/${id}`, options);

/**
 * Hand a flight to another user. Admin-only (`MANAGE_TRACKS`); a private wing
 * on the flight is carried across to the new owner server-side.
 */
export const transferFlight = (
  id: string,
  userId: number,
  options: ApiRequestOptions = {},
): Promise<void> => apiPostVoid(`/me/flights/${id}/owner`, { userId }, options);
