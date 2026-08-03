import { Modal, Skeleton } from 'antd';
import { useState } from 'react';

import { getUser } from '../../../../api/admin/users';
import type { User } from '../../../../api/admin/users.io';
import { LoadError } from '../../../../components/LoadError';
import {
  useAsync,
  useAsyncEffect,
  useEventHandler,
} from '../../../../core/hooks';
import { UserForm } from './UserForm';

interface UserFormModalProps {
  open: boolean;
  /** Id of the user being edited, or `null` to create a new one. */
  userId: number | null;
  onSaved: (user: User) => void;
  onClose: () => void;
}

export function UserFormModal({
  open,
  userId,
  onSaved,
  onClose,
}: UserFormModalProps) {
  return (
    <Modal
      title={userId === null ? 'Add user' : 'Edit user'}
      open={open}
      footer={null}
      // Remount the body on each open so the fetch re-runs for the current
      // target and the form re-reads its `initialValues`.
      destroyOnHidden
      onCancel={onClose}
      width={560}
    >
      {open && (
        <ModalBody userId={userId} onSaved={onSaved} onCancel={onClose} />
      )}
    </Modal>
  );
}

interface ModalBodyProps {
  userId: number | null;
  onSaved: (user: User) => void;
  onCancel: () => void;
}

/**
 * The list row is a trimmed projection, so editing fetches the full
 * {@link User} (profile, verified-at, source) before the form can bind to it.
 * Create skips the fetch entirely.
 */
function ModalBody({ userId, onSaved, onCancel }: ModalBodyProps) {
  const [user, setUser] = useState<User | null>(null);
  const [fetchUser, , error] = useAsync(getUser);
  const [retryToken, setRetryToken] = useState(0);

  useAsyncEffect(
    async (signal) => {
      if (userId === null) {
        return;
      }

      setUser(null);
      const next = await fetchUser(userId, { signal });
      if (!signal.aborted) {
        setUser(next);
      }
    },
    [userId, retryToken],
  );

  const retry = useEventHandler(() => setRetryToken((t) => t + 1));

  if (userId === null) {
    return <UserForm user={null} onSaved={onSaved} onCancel={onCancel} />;
  }

  if (user === null && error !== null) {
    return (
      <LoadError title="Couldn't load user" error={error} onRetry={retry} />
    );
  }

  if (user === null) {
    return <Skeleton active paragraph={{ rows: 8 }} />;
  }

  return <UserForm user={user} onSaved={onSaved} onCancel={onCancel} />;
}
