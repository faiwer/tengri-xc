import type { LatLng } from './geo/coordinates';

export const formatCoordinates = ({ lat, lng }: LatLng): string =>
  `${lat.toFixed(5)}, ${lng.toFixed(5)}`;
