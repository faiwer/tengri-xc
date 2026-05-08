/**
 * Reconstructed in-memory track.
 */
export interface Track {
  /** Unix epoch seconds at index 0. */
  startTime: number;

  /** Unix epoch seconds. */
  t: Uint32Array;
  /** E5 micro-degrees (degree = value / 1e5). */
  lat: Int32Array;
  /** E5 micro-degrees (degree = value / 1e5). */
  lng: Int32Array;
  /** Decimetres (metre = value / 10). */
  alt: Int32Array;
  /** Decimetres. `null` for GPS-only tracks (no barometer). */
  baroAlt: Int32Array | null;
  /**
   * True airspeed in km/h (integer). `null` when the source had no TAS data.
   * When present, aligned 1:1 with the position arrays.
   */
  tas: Uint16Array | null;
}

export class TrackDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'TrackDecodeError';
  }
}
