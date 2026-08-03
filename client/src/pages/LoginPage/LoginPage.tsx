import { useEffect } from 'react';
import { Navigate } from 'react-router';
import { useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import { useLogin } from '../../features/login';

/**
 * `/login` stays as a deep-link entry point, but the form itself now lives in
 * a global modal. Logged-in users go to their flights; everyone else lands on
 * the home page with the login modal opened.
 */
export function LoginPage() {
  const { me } = useIdentity();
  const { openModal } = useLogin();

  useEffect(() => {
    if (!me) {
      openModal();
    }
  }, [me, openModal]);

  return <Navigate to={me ? routes.flights() : routes.home()} replace />;
}
