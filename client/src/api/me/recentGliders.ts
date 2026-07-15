import { apiGet, type ApiRequestOptions } from '../core';
import { RecentGlidersIo, type RecentGlider } from './recentGliders.io';

/**
 * `GET /me/gliders/recent` — the four most-recently-flown distinct gliders for
 * the signed-in pilot, newest first. Intended for the upload flow's recent
 * glider quick-pick.
 */
export async function listRecentGliders(
  options: ApiRequestOptions = {},
): Promise<RecentGlider[]> {
  return apiGet('/me/gliders/recent', RecentGlidersIo, options);
}
