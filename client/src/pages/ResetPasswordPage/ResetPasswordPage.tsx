import { useState } from 'react';
import { PageLayout } from '../../components/PageLayout';
import { DeadLinkPanel } from './DeadLinkPanel';
import { MISSING_TOKEN, type DeadLink } from './deadLink';
import { NewPasswordForm } from './NewPasswordForm';
import styles from './ResetPasswordPage.module.scss';

/**
 * `/reset-password?token=…` — where the link in a reset mail lands. Setting the
 * password also signs the user in, so this drops the returned `Me` into the
 * identity context and continues to the settings page that lets them change it
 * again.
 */
export function ResetPasswordPage() {
  const token = new URLSearchParams(window.location.search).get('token');
  const [dead, setDead] = useState<DeadLink | null>(null);

  return (
    <PageLayout fit>
      <article className={styles.page}>
        <h1 className={styles.title}>Choose a new password</h1>
        {!token ? (
          <DeadLinkPanel dead={MISSING_TOKEN} />
        ) : dead ? (
          <DeadLinkPanel dead={dead} />
        ) : (
          <NewPasswordForm token={token} onDeadLink={setDead} />
        )}
      </article>
    </PageLayout>
  );
}
