import { z } from 'zod';
import type { Sport } from '../admin/gliders.io';
import { apiDelete, apiPostRaw, type ApiRequestOptions } from '../core';
import type { LaunchMethod, Propulsion } from '../flights.io';
import { gzipFlightFile } from '../tracks';

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

/**
 * Delete a flight and all its data. Allowed for the flight's owner or an admin
 * with `MANAGE_TRACKS`; the server cascades tracks/routes/scoring and reaps a
 * now-orphaned private glider.
 */
export const deleteFlight = (
  id: string,
  options: ApiRequestOptions = {},
): Promise<void> => apiDelete(`/me/flights/${id}`, options);
