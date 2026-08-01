import { createTengriDemLoader, type TengriDemLoader } from 'tengri-maplibre';

/** JAXA whole-world DEM archive; `maxZ`/`t` are mandatory (header isn't read). */
// TODO: Don't hardcode. Take it from the instance config.
export const DEM_SOURCE_URL =
  'tengri://https://maps.faiwer.dev/jaxa_world.tengri-map?maxZ=11&t=dem';

/**
 * A DEM loader over {@link DEM_SOURCE_URL}. The loader wrapper is cheap and
 * stateless; the range-request cache lives in the underlying `TengriArchive`,
 * which is shared per file URL, so the map terrain style and the flight chart
 * hit one LRU regardless of how many loaders are created.
 */
export const createDemLoader = (): TengriDemLoader =>
  createTengriDemLoader(DEM_SOURCE_URL);
