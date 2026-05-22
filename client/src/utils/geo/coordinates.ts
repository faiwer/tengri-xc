import { z } from 'zod';

export const E5CoordinateIo = z.number().int().brand<'E5Coordinate'>();
export type E5Coordinate = z.infer<typeof E5CoordinateIo>;

export const DecimalDegreeIo = z.number().brand<'DecimalDegree'>();
export type DecimalDegree = z.infer<typeof DecimalDegreeIo>;

export interface E5LatLon {
  /** E5 micro-degrees (degree = value / 1e5). */
  lat: E5Coordinate;
  /** E5 micro-degrees (degree = value / 1e5). */
  lon: E5Coordinate;
}

export interface LatLng {
  lat: DecimalDegree;
  lng: DecimalDegree;
}

export const E5_PER_DEGREE = 100_000;

export const e5Coordinate = (value: number): E5Coordinate =>
  value as E5Coordinate;

export const decimalDegree = (value: number): DecimalDegree =>
  value as DecimalDegree;

export const e5ToDegrees = (value: E5Coordinate): DecimalDegree =>
  (value / E5_PER_DEGREE) as DecimalDegree;

export const e5LatLonToLatLng = ({ lat, lon }: E5LatLon): LatLng => ({
  lat: e5ToDegrees(lat),
  lng: e5ToDegrees(lon),
});
