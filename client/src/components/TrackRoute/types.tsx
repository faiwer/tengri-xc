import type { DecimalDegree } from '../../utils/geo/coordinates';

export type RoutePoint = { lat: DecimalDegree; lng: DecimalDegree };

export type LegPath = [RoutePoint, RoutePoint];

export const samePoint = (a: RoutePoint, b: RoutePoint): boolean =>
  a.lat === b.lat && a.lng === b.lng;
