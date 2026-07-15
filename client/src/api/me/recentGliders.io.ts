import { z } from 'zod';

import { GliderClassIo, SportIo } from '../admin/gliders.io';
import { LaunchMethodIo } from '../flights.io';

/**
 * One of the caller's recently-flown gliders, deduped server-side over
 * `(brandId, kind, modelId)`.
 */
export const RecentGliderIo = z.object({
  brandId: z.string(),
  brandName: z.string(),
  kind: SportIo,
  modelId: z.string(),
  modelName: z.string(),
  class: GliderClassIo,
  /** Unix epoch seconds of the latest flight on this glider. */
  takeoffAt: z.number().int(),
  /** IANA timezone name of that latest flight. */
  takeoffTimezone: z.string(),
  launchMethod: LaunchMethodIo,
});
export type RecentGlider = z.infer<typeof RecentGliderIo>;

export const RecentGlidersIo = z.array(RecentGliderIo);
