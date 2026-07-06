import type { PointWaypoint, Route } from '../../api/tracks.io';
import { e5LatLonToLatLng } from '../../utils/geo/coordinates';
import { SCORED_COLOR, UNSCORED_COLOR } from './constants';
import { samePoint, type LegPath, type RoutePoint } from './types';

interface RouteGeometry {
  legs: GeoJSON.FeatureCollection<GeoJSON.LineString>;
  waypoints: RoutePoint[];
}

export const buildRouteGeometry = (route: Route): RouteGeometry => {
  const turnpoints: RoutePoint[] = route.turnpoints
    // TODO: Support cylinder and line waypoints.
    .filter((point): point is PointWaypoint => point.type === 'point')
    .map((point) => e5LatLonToLatLng(point.fix));

  const isOlcTriangle =
    route.subType === 'olc_open' || route.subType === 'olc_closed';
  const closureStart =
    isOlcTriangle && route.closure?.start.type === 'point'
      ? e5LatLonToLatLng(route.closure.start.fix)
      : null;
  const closureEnd =
    isOlcTriangle && route.closure?.end.type === 'point'
      ? e5LatLonToLatLng(route.closure.end.fix)
      : null;

  const legFeatures: GeoJSON.Feature<GeoJSON.LineString>[] = [
    ...consecutiveLegs(turnpoints).map((path) =>
      legFeature(path, 'solid', SCORED_COLOR),
    ),
    ...(isOlcTriangle ? triangleClosingLeg(turnpoints) : []).map((path) =>
      legFeature(path, 'dashed', SCORED_COLOR),
    ),
    ...unscoredFlownLegs(turnpoints, closureStart, closureEnd).map((path) =>
      legFeature(path, 'solid', UNSCORED_COLOR),
    ),
    ...auxiliaryClosureLeg(closureStart, closureEnd).map((path) =>
      legFeature(path, 'dashed', UNSCORED_COLOR),
    ),
  ];

  const waypoints = uniquePoints([closureStart, ...turnpoints, closureEnd]);

  return {
    legs: { type: 'FeatureCollection', features: legFeatures },
    waypoints,
  };
};

const legFeature = (
  path: LegPath,
  style: 'solid' | 'dashed',
  color: string,
): GeoJSON.Feature<GeoJSON.LineString> => ({
  type: 'Feature',
  geometry: {
    type: 'LineString',
    coordinates: [
      [path[0].lng, path[0].lat],
      [path[1].lng, path[1].lat],
    ],
  },
  properties: { style, color },
});

const consecutiveLegs = (points: RoutePoint[]): LegPath[] =>
  points.slice(1).map((point, idx) => [points[idx], point]);

const triangleClosingLeg = (points: RoutePoint[]): LegPath[] =>
  points.length >= 3 ? [[points[points.length - 1], points[0]]] : [];

const unscoredFlownLegs = (
  points: RoutePoint[],
  closureStart: RoutePoint | null,
  closureEnd: RoutePoint | null,
): LegPath[] => {
  if (points.length === 0) {
    return [];
  }

  return [
    closureStart && !samePoint(closureStart, points[0]!)
      ? [closureStart, points[0]]
      : null,
    closureEnd && !samePoint(closureEnd, points[points.length - 1]!)
      ? [points[points.length - 1], closureEnd]
      : null,
  ].filter((path): path is LegPath => path !== null);
};

const auxiliaryClosureLeg = (
  closureStart: RoutePoint | null,
  closureEnd: RoutePoint | null,
): LegPath[] =>
  closureStart && closureEnd && !samePoint(closureStart, closureEnd)
    ? [[closureStart, closureEnd]]
    : [];

const uniquePoints = (points: (RoutePoint | null)[]): RoutePoint[] => {
  const result: RoutePoint[] = [];
  // O(n^2) but with at most ~6 points, faster than a Set with its hashing cost.
  for (const point of points) {
    if (point && result.every((existing) => !samePoint(existing, point))) {
      result.push(point);
    }
  }
  return result;
};
