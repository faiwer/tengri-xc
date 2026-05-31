import type { Route } from '../../api/tracks.io';
import styles from './TrackMetaPanel.module.scss';
import { RouteTypeIcon } from '../icons/RouteTypeIcon';

interface RouteSwitcherProps {
  routes: Route[];
  selectedRoute: Route | null;
  onSelect: (route: Route) => void;
}

export function RouteSwitcher({
  routes,
  selectedRoute,
  onSelect,
}: RouteSwitcherProps) {
  if (routes.length <= 1) {
    return null;
  }

  return (
    <span className={styles.routeSwitcher} aria-label="Available routes">
      {routes.map((candidate) => (
        <button
          key={`${candidate.routeType}-${candidate.subType}`}
          type="button"
          className={
            candidate === selectedRoute
              ? styles.routeButtonActive
              : styles.routeButton
          }
          onClick={() => onSelect(candidate)}
        >
          <RouteTypeIcon kind={candidate.routeType} />
        </button>
      ))}
    </span>
  );
}
