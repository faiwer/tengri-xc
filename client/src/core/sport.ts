/**
 * Disciplines the portal covers. The flying sports — paragliding,
 * hang-gliding, sailplane — each have a canonical glider-model
 * dictionary. `'other'` is the bucket for tracks recorded by the same
 * apps but from non-flying activities (ski tours, hikes…);
 */
export const SPORTS = ['hg', 'pg', 'sp', 'other'] as const;
export type Sport = (typeof SPORTS)[number];

/** Sports with a curated glider catalog — `SPORTS` minus `'other'`. */
export const CATALOG_SPORTS = ['hg', 'pg', 'sp'] as const;
export type CatalogSport = (typeof CATALOG_SPORTS)[number];

export const isCatalogSport = (sport: Sport): sport is CatalogSport =>
  sport !== 'other';
