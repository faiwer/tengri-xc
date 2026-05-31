import type { Route } from '../../api/tracks.io';
import styles from './TrackMetaPanel.module.scss';

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
          {routeTypeLabel(candidate)}
        </button>
      ))}
    </span>
  );
}

const routeTypeLabel = (route: Route): string => {
  switch (route.routeType) {
    case 'free_distance':
      return 'FD';
    case 'free_triangle':
      return 'T';
    case 'fai_triangle':
      return 'FAI';
    case 'task':
      return 'Task';
  }
};
