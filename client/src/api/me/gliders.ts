import { apiGet, type ApiRequestOptions } from '../core';
import { MyGlidersIo, type MyGlider } from './gliders.io';

/**
 * `GET /me/gliders` — every distinct wing the signed-in pilot has flown, with a
 * `flightsCount` per row. Sorted server-side by kind, then brand, then model;
 * the page groups them client-side.
 */
export async function listMyGliders(
  options: ApiRequestOptions = {},
): Promise<MyGlider[]> {
  return apiGet('/me/gliders', MyGlidersIo, options);
}
