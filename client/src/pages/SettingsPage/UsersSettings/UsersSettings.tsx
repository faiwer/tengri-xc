import { PlusOutlined } from '@ant-design/icons';
import { Button, Input, Skeleton, Table } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useMemo, useState } from 'react';

import type { User, UserListItem } from '../../../api/admin/users.io';
import { Flag } from '../../../components/Flag';
import { LoadError } from '../../../components/LoadError';
import { useErrorToast, useEventHandler } from '../../../core/hooks';
import { usePreferences } from '../../../core/preferences';
import {
  formatShortDate,
  formatShortTime,
} from '../../../utils/formatDateTime';
import { SettingsSection } from '../SettingsSection';
import { UserFormModal } from './UserFormModal';
import { UserRowActions } from './UserRowActions';
import styles from './UsersSettings.module.scss';
import { useUsersFeed } from './useUsersFeed';
import { isAdminBits } from '../../../core/identity';

export function UsersSettings() {
  const feed = useUsersFeed();
  const prefs = usePreferences();
  useErrorToast(feed.error, { title: "Couldn't load users" });

  const [modalOpen, setModalOpen] = useState(false);
  // The id being edited, or `null` when the modal is opened to create.
  const [editingId, setEditingId] = useState<number | null>(null);

  const openCreate = () => {
    setEditingId(null);
    setModalOpen(true);
  };

  const openEdit = useEventHandler((user: UserListItem) => {
    setEditingId(user.id);
    setModalOpen(true);
  });

  const onSaved = (user: User) => {
    feed.onSaved(toListItem(user));
    setModalOpen(false);
  };

  const columns = useMemo<ColumnsType<UserListItem>>(
    () => [
      { title: 'ID', dataIndex: 'id', key: 'id', width: '48px' },
      {
        title: 'Name',
        dataIndex: 'name',
        key: 'name',
        ellipsis: true,
        render: (name: string, record) => (
          <>
            {record.country && (
              <>
                <Flag code={record.country} />
                &nbsp;&nbsp;
              </>
            )}
            {name}
          </>
        ),
      },
      {
        title: 'Admin',
        dataIndex: 'permissions',
        key: 'admin',
        width: '72px',
        align: 'center',
        render: (bits: number) => (isAdminBits(bits) ? '✔️' : null),
      },
      {
        title: 'Joined',
        dataIndex: 'createdAt',
        key: 'createdAt',
        width: '96px',
        render: (epoch: number) => formatShortDate(epoch, prefs),
      },
      {
        title: 'Last login',
        dataIndex: 'lastLoginAt',
        key: 'lastLoginAt',
        width: '160px',
        render: (epoch: number | null) =>
          epoch === null ? (
            <Muted>never</Muted>
          ) : (
            `${formatShortDate(epoch, prefs)} ${formatShortTime(epoch, prefs)}`
          ),
      },
      {
        key: 'actions',
        width: '80px',
        align: 'center',
        render: (_, record) => (
          <UserRowActions user={record} onEdit={openEdit} />
        ),
      },
    ],
    [prefs, openEdit],
  );

  // Inline error only for the empty/initial state — otherwise the
  // toast handles it and we keep showing the rows we already have.
  const hasInlineError = feed.error !== null && feed.items === null;

  return (
    <SettingsSection
      title="Users"
      scrollable
      action={
        <div className={styles.actions}>
          <Input.Search
            allowClear
            placeholder="Search by name, login, or email"
            value={feed.query}
            onChange={(e) => feed.setQuery(e.target.value)}
            className={styles.search}
          />
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={openCreate}
            aria-label="Add user"
            className={styles.actionBtn}
          />
        </div>
      }
    >
      {hasInlineError ? (
        <LoadError
          title="Couldn't load users"
          error={feed.error}
          onRetry={feed.retry}
        />
      ) : feed.items === null ? (
        <Skeleton active paragraph={{ rows: 6 }} />
      ) : (
        <>
          <Table
            rowKey="id"
            size="middle"
            tableLayout="fixed"
            columns={columns}
            dataSource={feed.items}
            pagination={false}
            loading={feed.isLoading && feed.items.length > 0}
            locale={{
              emptyText: feed.query
                ? `No users match "${feed.query}".`
                : 'No users yet.',
            }}
          />
          {!feed.completed && feed.items.length > 0 && (
            <div className={styles.loadMore}>
              <Button
                onClick={feed.loadMore}
                loading={feed.isLoading}
                disabled={feed.isLoading}
              >
                Load more
              </Button>
            </div>
          )}
        </>
      )}

      <UserFormModal
        open={modalOpen}
        userId={editingId}
        onSaved={onSaved}
        onClose={() => setModalOpen(false)}
      />
    </SettingsSection>
  );
}

/** Project the full `User` returned by create/update into a list row. */
const toListItem = (user: User): UserListItem => ({
  id: user.id,
  name: user.name,
  login: user.login,
  email: user.email,
  permissions: user.permissions,
  country: user.profile?.country ?? null,
  createdAt: user.createdAt,
  lastLoginAt: user.lastLoginAt,
});

const Muted = ({ children }: { children: React.ReactNode }) => (
  <span className={styles.muted}>{children}</span>
);
