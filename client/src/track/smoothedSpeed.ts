import { movingAverage } from '../utils/movingAverage';

export const AIRSPEED_SMOOTHING_HALF_SECONDS = 30;

export const smoothTrackSpeed = (
  times: Uint32Array,
  speed: Float32Array,
): Float32Array => movingAverage(times, speed, AIRSPEED_SMOOTHING_HALF_SECONDS);

export const smoothTas = (times: Uint32Array, tas: Uint16Array): Float32Array =>
  smoothTrackSpeed(times, tasAsFloat32(tas));

const tasAsFloat32 = (tas: Uint16Array): Float32Array => {
  const out = new Float32Array(tas.length);
  for (let i = 0; i < tas.length; i++) {
    out[i] = tas[i]!;
  }
  return out;
};
