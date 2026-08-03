import { DeleteOutlined, EditOutlined } from '@ant-design/icons';
import { Button } from 'antd';

import type { UserListItem } from '../../../api/admin/users.io';
import styles from './UserRowActions.module.scss';

interface UserRowActionsProps {
  user: UserListItem;
  onEdit: (user: UserListItem) => void;
}

export function UserRowActions({ user, onEdit }: UserRowActionsProps) {
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
        // TODO
        disabled
        icon={<DeleteOutlined />}
        aria-label={`Delete "${user.name}"`}
      />
    </div>
  );
}
