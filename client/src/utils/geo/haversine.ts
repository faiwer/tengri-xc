/**
 * Great-circle distance in metres between two points given as E5
 * micro-degrees (`degrees × 10⁵`, the project-wide wire unit).
 *
 * Mirrors `server/src/geo/haversine.rs`: spherical Earth at 6 371 km, no
 * altitude term, asin(sqrt(...)) form for sub-metre precision on adjacent
 * GPS fixes (vs `acos`, which collapses near 1).
 *
 * Worst-case error is ~0.3 % between equator and poles vs WGS-84, which
 * is well below the noise floor for ground-speed thresholding and per-leg
 * track distance.
 */
export const haversineM = (
  latAE5: number,
  lonAE5: number,
  latBE5: number,
  lonBE5: number,
): number => {
  const latA = latAE5 * E5_TO_RAD;
  const latB = latBE5 * E5_TO_RAD;
  const dlat = (latBE5 - latAE5) * E5_TO_RAD;
  const dlon = (lonBE5 - lonAE5) * E5_TO_RAD;
  const sLat = Math.sin(dlat * 0.5);
  const sLon = Math.sin(dlon * 0.5);
  const a = sLat * sLat + Math.cos(latA) * Math.cos(latB) * sLon * sLon;
  const c = 2 * Math.asin(Math.sqrt(a));
  return EARTH_RADIUS_M * c;
};

export const EARTH_RADIUS_M = 6_371_000;
const E5_TO_RAD = Math.PI / 180 / 1e5;
