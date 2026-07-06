import { useEffect } from 'react';
import { decimalDegree, type LatLng } from '../../utils/geo/coordinates';
import { useMap } from './hooks/useMap';

interface MapCenterReporterProps {
  onCenterLatLng: (point: LatLng) => void;
}

export function MapCenterReporter({ onCenterLatLng }: MapCenterReporterProps) {
  const map = useMap();

  useEffect(() => {
    const emitCenter = () => {
      const center = map.getCenter();
      onCenterLatLng({
        lat: decimalDegree(center.lat),
        lng: decimalDegree(center.lng),
      });
    };

    emitCenter();
    map.on('moveend', emitCenter);
    return () => {
      map.off('moveend', emitCenter);
    };
  }, [map, onCenterLatLng]);

  return null;
}
