import { App, Modal } from 'antd';
import { updateFlight } from '../../api/me/flights';
import type { TrackMetadata } from '../../api/tracks.io';
import { useAsync, useErrorToast } from '../../core/hooks';
import { isCatalogSport } from '../../core/sport';
import {
  FlightDetailsStep,
  type FlightDetails,
  type FlightDetailsForm,
} from '../../features/flightDetails';

interface EditFlightModalProps {
  open: boolean;
  /** The flight being edited; seeds the form with its current metadata. */
  flight: TrackMetadata;
  onClose: () => void;
  /** Receives the refreshed metadata the server returns after a save. */
  onSaved: (updated: TrackMetadata) => void;
}

export function EditFlightModal({
  open,
  flight,
  onClose,
  onSaved,
}: EditFlightModalProps) {
  const { notification } = App.useApp();

  const [save, isSaving, saveError] = useAsync(
    async (details: FlightDetails) => {
      const updated = await updateFlight(flight.id, details);
      notification.success({
        title: 'Flight updated',
        placement: 'bottomRight',
      });
      onSaved(updated);
    },
  );
  useErrorToast(saveError, { title: "Couldn't update flight" });

  return (
    <Modal
      title="Edit flight"
      open={open}
      footer={null}
      width={760}
      mask={{ closable: !isSaving }}
      onCancel={isSaving ? undefined : onClose}
    >
      <FlightDetailsStep
        // Remount on reopen / flight change so the form re-seeds from the
        // current metadata rather than keeping a stale draft.
        key={`${flight.id}:${String(open)}`}
        initial={initialForm(flight)}
        isSubmitting={isSaving}
        submitLabel="Save"
        onCancel={onClose}
        onSubmit={(details) => void save(details)}
      />
    </Modal>
  );
}

/** Seed the details form from a flight's current metadata. */
function initialForm(flight: TrackMetadata): FlightDetailsForm {
  const { glider, launchMethod, propulsion } = flight;
  // `KindSwitch` only offers the catalog sports (hg/pg/sp); an `other` glider
  // has no catalog, so fall back to `hg` and force a fresh brand/model pick.
  const inCatalog = isCatalogSport(glider.kind);
  return {
    kind: inCatalog ? glider.kind : 'hg',
    brandId: inCatalog ? glider.brandId : null,
    modelId: inCatalog ? glider.modelId : null,
    launchMethod,
    propulsion,
  };
}
