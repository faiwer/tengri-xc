import { useMap } from '@vis.gl/react-google-maps';
import { useEffect, useRef } from 'react';

interface TrackHoverMarkerProps {
  point: google.maps.LatLngLiteral | null;
}

/**
 * Pixel-sized hover marker for the chart-linked track position. Google Maps
 * circles use metre radii, so a symbol marker keeps the on-screen size stable
 * while the user zooms the map.
 */
export function TrackHoverMarker({ point }: TrackHoverMarkerProps) {
  const map = useMap();
  const markerRef = useRef<google.maps.Marker | null>(null);

  useEffect(() => {
    if (!map) {
      return;
    }

    const marker = new google.maps.Marker({
      icon: {
        path: google.maps.SymbolPath.CIRCLE,
        scale: MARKER_SCALE,
        strokeColor: STROKE_COLOR,
        strokeOpacity: 1,
        strokeWeight: 2,
        fillColor: FILL_COLOR,
        fillOpacity: 0.9,
      },
      map,
      clickable: false,
      zIndex: 20,
    });
    markerRef.current = marker;

    return () => {
      marker.setMap(null);
      markerRef.current = null;
    };
  }, [map]);

  useEffect(() => {
    const marker = markerRef.current;
    if (marker) {
      marker.setPosition(point);
      marker.setVisible(point !== null);
    }
  }, [point]);

  return null;
}

const STROKE_COLOR = '#2958a1';
const FILL_COLOR = 'white';
const MARKER_SCALE = 7;
