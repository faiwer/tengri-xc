import { useMap } from '@vis.gl/react-google-maps';
import { useEffect } from 'react';
import type { PointWaypoint, Route } from '../../api/tracks.io';
import { e5LatLonToLatLng } from '../../utils/geo/coordinates';
import { MAP_Z_INDEX } from './zIndex';

interface TrackRouteProps {
  route: Route;
}

export function TrackRoute({ route }: TrackRouteProps) {
  const map = useMap();

  useEffect(() => {
    if (!map) {
      return;
    }

    const turnpoints = route.turnpoints
      // TODO: Support cylinder and line waypoints.
      .filter((point): point is PointWaypoint => point.type === 'point')
      .map((point) => e5LatLonToLatLng(point.fix));

    const isOlcTriangle = isOlcTriangleRoute(route);
    const closureStart =
      isOlcTriangle && route.closure?.start.type === 'point'
        ? e5LatLonToLatLng(route.closure.start.fix)
        : null;
    const closureEnd =
      isOlcTriangle && route.closure?.end.type === 'point'
        ? e5LatLonToLatLng(route.closure.end.fix)
        : null;

    const legs = [
      ...consecutiveLegs(turnpoints).map((path) =>
        solidLeg(path, SCORED_COLOR),
      ),
      ...(isOlcTriangle ? triangleClosingLeg(turnpoints) : []).map((path) =>
        dashedLeg(
          path,
          SCORED_COLOR,
          DOTTED_LEG_DASH_LENGTH,
          DOTTED_LEG_REPEAT,
        ),
      ),
      ...unscoredFlownLegs(turnpoints, closureStart, closureEnd).map((path) =>
        solidLeg(path, UNSCORED_COLOR),
      ),
      ...auxiliaryClosureLeg(closureStart, closureEnd).map((path) =>
        dashedLeg(
          path,
          UNSCORED_COLOR,
          DOTTED_LEG_DASH_LENGTH,
          DOTTED_LEG_REPEAT,
        ),
      ),
    ].map((options) => new google.maps.Polyline({ ...options, map }));

    const markers = uniquePoints([closureStart, ...turnpoints, closureEnd]).map(
      (position) =>
        new google.maps.Marker({
          position,
          icon: {
            path: google.maps.SymbolPath.CIRCLE,
            scale: MARKER_SCALE,
            strokeColor: MARKER_STROKE_COLOR,
            strokeOpacity: 1,
            strokeWeight: 2,
            fillColor: MARKER_FILL_COLOR,
            fillOpacity: 1,
          },
          map,
          clickable: false,
          zIndex: MAP_Z_INDEX.routeWaypoints,
        }),
    );

    return () => {
      for (const leg of legs) {
        leg.setMap(null);
      }

      for (const marker of markers) {
        marker.setMap(null);
      }
    };
  }, [map, route]);

  return null;
}

type RoutePoint = google.maps.LatLngLiteral;
type LegPath = [RoutePoint, RoutePoint];

const isOlcTriangleRoute = (route: Route): boolean =>
  route.subType === 'olc_open' || route.subType === 'olc_closed';

const consecutiveLegs = (points: RoutePoint[]): LegPath[] =>
  points.slice(1).map((point, idx) => [points[idx], point]);

const triangleClosingLeg = (points: RoutePoint[]): LegPath[] =>
  points.length >= 3 ? [[points[points.length - 1]!, points[0]!]] : [];

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
      ? ([closureStart, points[0]!] as LegPath)
      : null,
    closureEnd && !samePoint(closureEnd, points[points.length - 1]!)
      ? ([points[points.length - 1]!, closureEnd] as LegPath)
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

const solidLeg = (
  path: LegPath,
  color: string,
): google.maps.PolylineOptions => ({
  path,
  strokeColor: color,
  strokeOpacity: 1,
  strokeWeight: 3,
  clickable: false,
  zIndex: MAP_Z_INDEX.routeLegs,
});

const dashedLeg = (
  path: LegPath,
  color: string,
  dashLength: number,
  repeat: string,
): google.maps.PolylineOptions => ({
  path,
  strokeColor: color,
  strokeOpacity: 0,
  strokeWeight: 3,
  clickable: false,
  zIndex: MAP_Z_INDEX.routeLegs,
  icons: [
    // A special hack to make the line dashed.
    {
      icon: {
        path: 'M 0,-1 0,1',
        scale: dashLength,
        strokeColor: color,
        strokeOpacity: 1,
        strokeWeight: DOTTED_LEG_STROKE_WIDTH,
      },
      offset: '0',
      repeat,
    },
  ],
});

// Just in case — skip showing the same marker twice.
const uniquePoints = (points: (RoutePoint | null)[]): RoutePoint[] => {
  const result: RoutePoint[] = [];
  // O(n^2) but still faster than a Set with its O(1) lookup.
  for (const point of points) {
    if (point && result.every((existing) => !samePoint(existing, point))) {
      result.push(point);
    }
  }
  return result;
};

const samePoint = (a: RoutePoint, b: RoutePoint): boolean =>
  a.lat === b.lat && a.lng === b.lng;

const SCORED_COLOR = '#65c832';
const UNSCORED_COLOR = '#d89a12';
const MARKER_STROKE_COLOR = SCORED_COLOR;
const MARKER_FILL_COLOR = 'white';
const MARKER_SCALE = 4;
const DOTTED_LEG_STROKE_WIDTH = 3;
const DOTTED_LEG_DASH_LENGTH = 4;
const DOTTED_LEG_REPEAT = '14px';
