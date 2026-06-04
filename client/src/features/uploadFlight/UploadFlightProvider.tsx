import {
  createContext,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useAsyncEffect, useEventHandler } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import { nullthrows } from '../../utils/nullthrows';
import { UploadFlightModal } from './UploadFlightModal';

export interface UploadFlightContextValue {
  openModal: () => void;
}

const UploadFlightContext = createContext<UploadFlightContextValue | null>(
  null,
);

interface UploadFlightProviderProps {
  children: ReactNode;
}

export function UploadFlightProvider({ children }: UploadFlightProviderProps) {
  const { me } = useIdentity();
  const [isOpen, setIsOpen] = useState(false);
  const openModal = useEventHandler(() => setIsOpen(true));
  const closeModal = useEventHandler(() => setIsOpen(false));

  useOpenModalOnDocumentFileDrag({
    enabled: !!me,
    openModal,
  });

  return (
    <UploadFlightContext.Provider
      value={useMemo<UploadFlightContextValue>(
        () => ({ openModal }),
        [openModal],
      )}
    >
      {children}
      <UploadFlightModal open={isOpen} onClose={closeModal} />
    </UploadFlightContext.Provider>
  );
}

export function useUploadFlight(): UploadFlightContextValue {
  return nullthrows(
    useContext(UploadFlightContext),
    'useUploadFlight must be used inside an <UploadFlightProvider>',
  );
}

function useOpenModalOnDocumentFileDrag({
  enabled,
  openModal,
}: {
  enabled: boolean;
  openModal: () => void;
}): void {
  useAsyncEffect(() => {
    if (!enabled) return;

    const onDrag = (event: DragEvent) => {
      const items = [...((event.dataTransfer ?? {})?.items ?? [])];
      if (items.length == 1 && items[0]?.kind === 'file') {
        openModal();
        // By default the browser highlights the modal.
        (document.activeElement as HTMLElement)?.blur();
      }
    };

    document.addEventListener('dragenter', onDrag, true);
    return () => {
      document.removeEventListener('dragenter', onDrag, true);
    };
  }, [enabled]);
}
