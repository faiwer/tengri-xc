import { z } from 'zod';

/** One row of `GET /admin/sites`. `country` drives the flag in the Name cell. */
export const SiteListItemIo = z.object({
  id: z.number().int(),
  name: z.string(),
  /** ISO 3166-1 alpha-2, or `null` when unset. */
  country: z.string().nullable(),
  /** Decimal degrees on WGS-84. */
  lat: z.number(),
  lng: z.number(),
});

export type SiteListItem = z.infer<typeof SiteListItemIo>;

export const SitesPageIo = z.object({
  items: z.array(SiteListItemIo),
  /** Opaque cursor for the next page; `null` on the last page. */
  nextCursor: z.string().nullable(),
});

export type SitesPage = z.infer<typeof SitesPageIo>;

/** Result of `POST /admin/sites/reindex`. */
export const ReindexResultIo = z.object({
  /** Number of flights whose closest-takeoff link was recomputed. */
  updated: z.number().int(),
});

export type ReindexResult = z.infer<typeof ReindexResultIo>;

/** Body for create (`POST /admin/sites`) and update (`PATCH /admin/sites/:id`). */
export interface SiteInput {
  name: string;
  /** Decimal degrees on WGS-84. */
  lat: number;
  lng: number;
  /** ISO 3166-1 alpha-2, or `null` when unset. */
  country: string | null;
}
