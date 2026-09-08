import { Divider, Modal, Typography } from 'antd';
import { useAsyncData } from '../../core/hooks';
import { useSite } from '../../core/site';
import styles from './LoginModal.module.scss';
import { getEnabledProviders } from '../../api/oauth';
import { OAuthRow } from '../oauth/OAuthRow';
import { SignInForm } from './SignInForm';
import { useLayoutEffect, useState } from 'react';
import { RegisterForm } from './RegisterForm';

interface LoginModalProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Username-or-email + password modal. On success: store the `Me` returned by
 * the server in the identity context and close. The current page re-renders
 * with the new identity; the session cookie is set by the server (HttpOnly).
 */
export function LoginModal({ open, onClose }: LoginModalProps) {
  const [form, setForm] = useState<'login' | 'reset' | 'register'>('login');
  const providers = useAsyncData(
    (signal) => getEnabledProviders({ signal }),
    [],
  );
  const [formKey, setFormKey] = useState(0);

  useLayoutEffect(() => {
    if (open) {
      setForm('login');
      setFormKey((k) => k + 1); // Re-mount the form component.
    }
  }, [open]);

  return (
    <Modal
      title={
        form === 'login'
          ? 'Sign in'
          : form === 'register'
            ? 'New account'
            : 'Reset password'
      }
      open={open}
      footer={null}
      width={400}
      onCancel={onClose}
      className={styles.modal}
    >
      {form === 'login' && (
        <>
          <SignInForm key={formKey} onClose={onClose} />
          <SignInFooter setForm={setForm} />
          {providers.data && (
            <>
              <Divider plain>or</Divider>
              <OAuthRow
                providerIds={providers.data}
                intent="login"
                align="center"
              />
            </>
          )}
        </>
      )}

      {form === 'register' && <RegisterForm onClose={onClose} />}
    </Modal>
  );
}

function SignInFooter({
  setForm,
}: {
  setForm: (form: 'reset' | 'register') => void;
}) {
  const { canRegister } = useSite().site;

  return (
    <ul className={styles.footer}>
      {canRegister && (
        <li>
          No account?{' '}
          <Typography.Link onClick={() => setForm('register')}>
            Register
          </Typography.Link>
        </li>
      )}
      <li>
        Forgot your password?{' '}
        <Typography.Link onClick={() => setForm('reset')}>
          Reset
        </Typography.Link>
      </li>
    </ul>
  );
}
