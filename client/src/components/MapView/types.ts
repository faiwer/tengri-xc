import { z } from 'zod';

export const MAP_TYPE_SCHEMA = z.enum([
  'roadmap',
  'terrain',
  'satellite',
  'hybrid',
]);
export type MapType = z.infer<typeof MAP_TYPE_SCHEMA>;
