import { APIProvider, Map } from '@vis.gl/react-google-maps';
import styles from './MapView.module.scss';

const API_KEY = import.meta.env.VITE_GOOGLE_MAPS_API_KEY;

const DEFAULT_CENTER = { lat: 0, lng: 0 };
const DEFAULT_ZOOM = 2;

export function MapView() {
  return (
    <div className={styles.container}>
      <APIProvider apiKey={API_KEY}>
        <Map
          className={styles.map}
          defaultCenter={DEFAULT_CENTER}
          defaultZoom={DEFAULT_ZOOM}
          gestureHandling="greedy"
          disableDefaultUI={false}
        />
      </APIProvider>
    </div>
  );
}
