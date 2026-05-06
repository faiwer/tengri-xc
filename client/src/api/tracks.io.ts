import { z } from 'zod';

export const TrackMetadataIo = z.object({
  id: z.string(),
  pilot: z.object({
    name: z.string(),
  }),
});

export type TrackMetadata = z.infer<typeof TrackMetadataIo>;
