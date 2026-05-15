import { useMap } from '@vis.gl/react-google-maps';
import { useEffect } from 'react';
import { useEventHandler } from '../../core/hooks';
import { nullthrows } from '../../utils/nullthrows';

interface MapCenterReporterProps {
  onCenterLatLng: (point: google.maps.LatLngLiteral) => void;
}

export function MapCenterReporter({ onCenterLatLng }: MapCenterReporterProps) {
  const map = nullthrows(useMap());
  const emitCenter = useEventHandler(() => {
    onCenterLatLng(nullthrows(map.getCenter()).toJSON());
  });

  useEffect(() => {
    emitCenter();
    const listener = map.addListener('idle', emitCenter);
    return () => {
      listener.remove();
    };
  }, [map, emitCenter]);

  return null;
}
