import {
  createContext,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useEventHandler } from '../../core/hooks';
import { nullthrows } from '../../utils/nullthrows';
import { LoginModal } from './LoginModal';

export interface LoginContextValue {
  openModal: () => void;
}

const LoginContext = createContext<LoginContextValue | null>(null);

interface LoginProviderProps {
  children: ReactNode;
}

export function LoginProvider({ children }: LoginProviderProps) {
  const [isOpen, setIsOpen] = useState(false);
  const openModal = useEventHandler(() => setIsOpen(true));
  const closeModal = useEventHandler(() => setIsOpen(false));

  return (
    <LoginContext.Provider
      value={useMemo<LoginContextValue>(() => ({ openModal }), [openModal])}
    >
      {children}
      <LoginModal open={isOpen} onClose={closeModal} />
    </LoginContext.Provider>
  );
}

export function useLogin(): LoginContextValue {
  return nullthrows(
    useContext(LoginContext),
    'useLogin must be used inside a <LoginProvider>',
  );
}
