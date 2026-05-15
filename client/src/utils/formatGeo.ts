export const formatCoordinates = ({
  lat,
  lng,
}: google.maps.LatLngLiteral): string => `${lat.toFixed(5)}, ${lng.toFixed(5)}`;
