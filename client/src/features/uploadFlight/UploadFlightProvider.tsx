import {
  createContext,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useEventHandler } from '../../core/hooks';
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
  const [isOpen, setIsOpen] = useState(false);
  const openModal = useEventHandler(() => setIsOpen(true));
  const closeModal = useEventHandler(() => setIsOpen(false));

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
