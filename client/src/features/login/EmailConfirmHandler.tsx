import { App } from 'antd';
import { useAsyncEffect } from '../../core/hooks';
import { useIdentity } from '../../core/identity';

/**
 * Reads the `?email` / `?email_error` status `GET /users/confirm-email` appends
 * when it bounces the browser back from the link in a confirmation mail, toasts
 * it, then strips the param so a refresh doesn't re-toast. `?email=confirmed`
 * finished a registration, `?email=changed` an address change.
 *
 * Rendered once near the app root; no UI of its own.
 */
export function EmailConfirmHandler() {
  const { notification } = App.useApp();
  const { retry } = useIdentity();

  useAsyncEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const ok = params.get(OK_PARAM);
    const err = params.get(ERR_PARAM);
    if (!ok && !err) {
      return;
    }

    if (ok) {
      notification.success({
        title: ok === 'changed' ? 'Email address updated' : 'Email confirmed',
        description: 'You are signed in.',
        placement: 'bottomRight',
      });
      // The redirect set the session cookie server-side, so the boot probe
      // that already ran saw an anonymous visitor.
      retry();
    } else if (err) {
      notification.error({
        title: "Couldn't confirm your email",
        description: ERRORS[err] ?? 'Please try again.',
        placement: 'bottomRight',
      });
    }

    params.delete(OK_PARAM);
    params.delete(ERR_PARAM);
    const query = params.toString();
    window.history.replaceState(
      null,
      '',
      window.location.pathname + (query ? `?${query}` : ''),
    );
  }, []);

  return null;
}

const OK_PARAM = 'email';
const ERR_PARAM = 'email_error';

const ERRORS: Record<string, string> = {
  expired: 'That link has expired. Links are valid for 24 hours.',
  invalid: "That link isn't valid. It may have already been replaced.",
  taken: 'Another account confirmed that address first. Try a different one.',
};
