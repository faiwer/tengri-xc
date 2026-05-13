/**
 * Disciplines the portal covers. The flying sports — paragliding,
 * hang-gliding, sailplane — each have a canonical glider-model
 * dictionary. `'other'` is the bucket for tracks recorded by the same
 * apps but from non-flying activities (ski tours, hikes…);
 */
export const SPORTS = ['hg', 'pg', 'sp', 'other'] as const;
export type Sport = (typeof SPORTS)[number];
