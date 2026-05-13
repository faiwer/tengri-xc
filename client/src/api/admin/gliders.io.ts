import { z } from 'zod';

import { SPORTS, type Sport } from '../../core/sport';

/**
 * Zod for {@link Sport}. The admin glider-dictionary endpoint narrows further
 * server-side (rejects `'other'`, which has no model list); this schema is the
 * broad one used for FE-wide things like persisted filter state.
 */
export const SportIo = z.enum(SPORTS);
export type { Sport };

/**
 * Polymorphic `glider_class` enum. The DB pins each value to a compatible
 * `kind` via CHECK constraints, so the values you get back from a request
 * scoped to one `kind` are already a single coherent subset.
 */
export const GliderClassIo = z.enum([
  'en_a',
  'en_b',
  'en_c',
  'en_d',
  'ccc',
  'single_surface',
  'kingpost',
  'topless',
  'rigid',
  'standard',
  'fifteen_metre',
  'eighteen_metre',
  'twenty_metre_two_seater',
  'open',
  'club',
  'motorglider',
]);
export type GliderClass = z.infer<typeof GliderClassIo>;

export const GliderBrandIo = z.object({
  id: z.string(),
  name: z.string(),
});
export type GliderBrand = z.infer<typeof GliderBrandIo>;

/**
 * One model in the per-kind catalog. `kind` is implied by the request (no
 * field), since the endpoint returns a single kind at a time.
 */
export const GliderModelIo = z.object({
  brandId: z.string(),
  id: z.string(),
  name: z.string(),
  class: GliderClassIo,
  isTandem: z.boolean(),
});
export type GliderModel = z.infer<typeof GliderModelIo>;

/**
 * One sport's glider catalog from `GET /admin/gliders?kind=...`. `brands` is
 * pre-filtered to brands with at least one model of the requested kind.
 */
export const GliderCatalogIo = z.object({
  brands: z.array(GliderBrandIo),
  models: z.array(GliderModelIo),
});
export type GliderCatalog = z.infer<typeof GliderCatalogIo>;
