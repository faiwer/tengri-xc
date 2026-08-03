import { DeleteOutlined, EditOutlined } from '@ant-design/icons';
import { App, Button } from 'antd';

import { deleteUser } from '../../../api/admin/users';
import type { UserListItem } from '../../../api/admin/users.io';
import styles from './UserRowActions.module.scss';

interface UserRowActionsProps {
  user: UserListItem;
  onEdit: (user: UserListItem) => void;
  onRemoved: (id: number) => void;
}

export function UserRowActions({
  user,
  onEdit,
  onRemoved,
}: UserRowActionsProps) {
  const { modal, notification } = App.useApp();

  const onDelete = () => {
    modal.confirm({
      title: `Delete "${user.name}"?`,
      content: confirmContent(user.flightCount),
      okText: 'Delete',
      okButtonProps: { danger: true },
      // Returning the promise keeps the OK button in its loading state until
      // the request settles, and re-throwing on failure leaves the modal open.
      onOk: async () => {
        try {
          await deleteUser(user.id);
          onRemoved(user.id);
          notification.success({
            title: `Deleted "${user.name}"`,
            placement: 'bottomRight',
          });
        } catch (err) {
          notification.error({
            title: "Couldn't delete user",
            description: err instanceof Error ? err.message : String(err),
            placement: 'bottomRight',
          });
          throw err;
        }
      },
    });
  };

  return (
    <div className={styles.actions}>
      <Button
        type="text"
        size="small"
        icon={<EditOutlined />}
        aria-label={`Edit "${user.name}"`}
        onClick={() => onEdit(user)}
      />
      <Button
        type="text"
        size="small"
        danger
        icon={<DeleteOutlined />}
        aria-label={`Delete "${user.name}"`}
        onClick={onDelete}
      />
    </div>
  );
}

const confirmContent = (flightCount: number): string => {
  const base = 'This permanently deletes the user';
  return flightCount > 0
    ? `${base} and all ${flightCount} flight(s) they own (tracks, routes, scores). This can't be undone.`
    : `${base}. This can't be undone.`;
};
