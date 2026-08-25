import { App } from 'antd';
import { useNavigate } from 'react-router';
import { useAsyncEffect } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';

/**
 * Reads the `?oauth` / `?oauth_error` status the OAuth callback appends when it
 * bounces the browser back, toasts it, then strips the param from the URL so a
 * refresh or share doesn't re-toast. A successful login also refetches identity
 * (the session cookie was just set server-side).
 *
 * Rendered once near the app root; no UI of its own.
 */
export function OAuthReturnHandler() {
  const { notification } = App.useApp();
  const { retry } = useIdentity();
  const navigate = useNavigate();

  useAsyncEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const ok = params.get(OK_PARAM);
    const err = params.get(ERR_PARAM);
    if (!ok && !err) {
      return;
    }

    if (ok) {
      notification.success({
        title: SUCCESS[ok] ?? 'Done',
        placement: 'bottomRight',
      });

      // Login and registration both just set the session cookie; refetch who we are.
      if (ok === 'logged_in' || ok === 'registered') {
        retry();
      }

      // A brand-new account lands on the profile page to fill in its details.
      // The clean route drops the callback params, so skip the manual strip.
      if (ok === 'registered') {
        navigate(routes.settings.profile(), { replace: true });
        return;
      }
    } else if (err) {
      notification.error({
        title: 'Social sign-in failed',
        description: ERRORS[err] ?? 'Please try again.',
        placement: 'bottomRight',
      });
    }

    // Clear the params from the URL. It's a one-time callback.
    params.delete(OK_PARAM);
    params.delete(ERR_PARAM);
    const query = params.toString();
    const url = window.location.pathname + (query ? `?${query}` : '');
    window.history.replaceState(null, '', url);
  }, []);

  return null;
}

const OK_PARAM = 'oauth';
const ERR_PARAM = 'oauth_error';

const SUCCESS: Record<string, string> = {
  linked: 'Account linked',
  logged_in: 'Signed in',
  registered: 'Welcome',
};

const ERRORS: Record<string, string> = {
  registration_disabled: 'Registration is currently disabled.',
  link_taken: 'That account is already linked to a different Tengri user.',
  banned: 'This account is banned and cannot sign in.',
  denied: 'Authorization was cancelled.',
  bad_state: 'The sign-in link expired. Please try again.',
  failed: 'Something went wrong during authorization.',
};
