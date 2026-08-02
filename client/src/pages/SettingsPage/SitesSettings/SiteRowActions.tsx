import { DeleteOutlined, EditOutlined } from '@ant-design/icons';
import { Button, Popconfirm } from 'antd';

import type { SiteListItem } from '../../../api/admin/sites.io';
import styles from './SiteRowActions.module.scss';

interface SiteRowActionsProps {
  site: SiteListItem;
  /** Whether this row's delete request is in flight (drives the confirm spinner). */
  deleting: boolean;
  onEdit: (site: SiteListItem) => void;
  onDelete: (site: SiteListItem) => void;
}

export function SiteRowActions({
  site,
  deleting,
  onEdit,
  onDelete,
}: SiteRowActionsProps) {
  return (
    <div className={styles.actions}>
      <Button
        type="text"
        size="small"
        icon={<EditOutlined />}
        aria-label={`Edit "${site.name}"`}
        onClick={() => onEdit(site)}
      />
      <Popconfirm
        title="Delete this site?"
        okText="Delete"
        okButtonProps={{ danger: true, loading: deleting }}
        onConfirm={() => onDelete(site)}
      >
        <Button
          type="text"
          size="small"
          danger
          icon={<DeleteOutlined />}
          aria-label={`Delete "${site.name}"`}
        />
      </Popconfirm>
    </div>
  );
}
