import { App } from 'antd';
import { useNavigate } from 'react-router';
import { deleteFlight } from '../../api/me/flights';
import { routes } from '../../core/routes';

/**
 * Returns a callback that confirms, deletes the flight, and leaves the now-gone
 * detail page. It replaces the current entry with the flights list so Back
 * can't return to the deleted flight.
 */
export function useRemoveFlight(flightId: string): () => void {
  const { modal, notification } = App.useApp();
  const navigate = useNavigate();

  return function removeFlight() {
    modal.confirm({
      title: 'Remove this flight?',
      content: 'This permanently deletes the flight and all its data.',
      okText: 'Remove',
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          await deleteFlight(flightId);
          notification.success({
            title: 'Flight removed',
            placement: 'bottomRight',
          });
          navigate(routes.flights(), { replace: true });
        } catch (err) {
          notification.error({
            title: "Couldn't remove flight",
            description: err instanceof Error ? err.message : String(err),
            placement: 'bottomRight',
          });
          throw err;
        }
      },
    });
  };
}
