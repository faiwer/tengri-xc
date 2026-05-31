import { describe, expect, it } from 'vitest';
import { TrackMetadataIo, TrackListItemIo } from '../tracks.io';

describe('track API schemas', () => {
  it('derives metadata offsets from timezone names', () => {
    const parsed = TrackMetadataIo.parse({
      id: 'track-1',
      pilot: { name: 'Pilot', country: null },
      glider: {
        brandId: 'aeros',
        brandName: 'Aeros',
        modelId: 'target',
        modelName: 'Target',
      },
      takeoffAt: Date.UTC(2025, 6, 15, 12) / 1000,
      landingAt: Date.UTC(2025, 6, 15, 12) / 1000,
      takeoffTimezone: 'Europe/Vienna',
      landingTimezone: 'Asia/Almaty',
      takeoff: { lat: 48.21, lon: 16.37 },
      landing: { lat: 43.25, lon: 76.95 },
      compressionRatio: 1,
      routes: [
        {
          id: 1,
          flightId: 'track-1',
          routeType: 'free_distance',
          subType: 'none',
          turnpoints: [
            {
              type: 'point',
              fix: {
                time: 1,
                lat: 4800000,
                lon: 1600000,
                geoAlt: 10000,
                pressureAlt: null,
                tas: null,
              },
            },
          ],
          legDistances: [],
          distance: 1234,
          score: 1.23,
          factor: 1,
          optimal: true,
          closure: null,
          scoredMs: 5,
        },
      ],
      mainRoute: {
        id: 1,
        routeType: 'free_distance',
        score: 1.23,
        distance: 1234,
      },
    });

    expect(parsed.takeoffOffset).toBe(2 * 3600);
    expect(parsed.landingOffset).toBe(5 * 3600);
    expect(parsed.routes[0]?.routeType).toBe('free_distance');
  });

  it('derives list offsets without changing the track shape consumed by the UI', () => {
    const parsed = TrackListItemIo.parse({
      pilot: { id: 1, name: 'Pilot', country: null },
      track: {
        id: 'track-1',
        takeoffAt: Date.UTC(2025, 6, 15, 12) / 1000,
        duration: 3600,
        takeoffTimezone: 'Europe/Vienna',
        landingTimezone: 'Asia/Almaty',
        takeoff: { lat: 48.21, lon: 16.37 },
        landing: { lat: 43.25, lon: 76.95 },
        mainRouteType: 'free_distance',
        mainScore: 1.23,
        mainDistance: 1234,
      },
    });

    expect(parsed.track.takeoffOffset).toBe(2 * 3600);
    expect(parsed.track.landingOffset).toBe(5 * 3600);
  });
});
