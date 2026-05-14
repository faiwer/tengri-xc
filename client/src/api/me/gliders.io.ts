import { z } from 'zod';

import { GliderClassIo, SportIo } from '../admin/gliders.io';

/**
 * One distinct wing the signed-in pilot has flown, aggregated by the server
 * over `flights` rows on `(brandId, kind, modelId)`. Always resolved — every
 * flight points at a `models` row (canonical or pilot-custom), so there's no
 * nullable brand/model fallback anymore.
 *
 * `private` is `true` when the model is a per-pilot custom (`models.user_id IS
 * NOT NULL`) and `false` for canonical rows from the curated catalog. `class`
 * may be `'unknown'` on customs the importer couldn't classify (HG-flex subtype
 * ambiguity, SP).
 */
export const MyGliderIo = z.object({
  brandId: z.string(),
  brandName: z.string(),
  kind: SportIo,
  modelId: z.string(),
  modelName: z.string(),
  class: GliderClassIo,
  isTandem: z.boolean(),
  private: z.boolean(),
  flightsCount: z.number().int().positive(),
});
export type MyGlider = z.infer<typeof MyGliderIo>;

export const MyGlidersIo = z.array(MyGliderIo);
