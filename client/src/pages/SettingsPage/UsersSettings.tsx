import { Alert, Button, Input, Skeleton, Table } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useMemo } from 'react';
import { useNavigate } from 'react-router';

import type { UserListItem } from '../../api/admin/users.io';
import { useErrorToast } from '../../core/hooks';
import { isAdminBits } from '../../core/identity';
import { usePreferences } from '../../core/preferences';
import { routes } from '../../core/routes';
import { formatShortDate } from '../../utils/formatDateTime';
import { SettingsSection } from './SettingsSection';
import styles from './UsersSettings.module.scss';
import { useUsersFeed } from './useUsersFeed';

export function UsersSettings() {
  const feed = useUsersFeed();
  const prefs = usePreferences();
  const navigate = useNavigate();
  useErrorToast(feed.error, { title: "Couldn't load users" });

  const columns = useMemo<ColumnsType<UserListItem>>(
    () => [
      {
        title: 'ID',
        dataIndex: 'id',
        key: 'id',
        width: 48,
      },
      {
        title: 'Name',
        dataIndex: 'name',
        key: 'name',
        ellipsis: true,
      },
      {
        title: 'Admin',
        dataIndex: 'permissions',
        key: 'admin',
        width: 72,
        align: 'center',
        render: (bits: number) => (isAdminBits(bits) ? '✔️' : null),
      },
      {
        title: 'Joined',
        dataIndex: 'createdAt',
        key: 'createdAt',
        width: 96,
        render: (epoch: number) => formatShortDate(epoch, prefs),
      },
      {
        title: 'Last login',
        dataIndex: 'lastLoginAt',
        key: 'lastLoginAt',
        width: 100,
        render: (epoch: number | null) =>
          epoch === null ? <Muted>never</Muted> : formatShortDate(epoch, prefs),
      },
    ],
    [prefs],
  );

  // Inline error only for the empty/initial state — otherwise the
  // toast handles it and we keep showing the rows we already have.
  const hasInlineError = feed.error !== null && feed.items === null;

  return (
    <SettingsSection
      title="Users"
      action={
        <Input.Search
          allowClear
          placeholder="Search by name, login, or email"
          value={feed.query}
          onChange={(e) => feed.setQuery(e.target.value)}
          className={styles.search}
        />
      }
    >
      {hasInlineError ? (
        <Alert
          type="error"
          showIcon
          title="Couldn't load users"
          description={feed.error}
          action={
            <Button size="small" onClick={() => window.location.reload()}>
              Reload
            </Button>
          }
        />
      ) : feed.items === null ? (
        <Skeleton active paragraph={{ rows: 6 }} />
      ) : (
        <>
          <Table
            rowKey="id"
            size="middle"
            columns={columns}
            dataSource={feed.items}
            pagination={false}
            loading={feed.isLoading && feed.items.length > 0}
            onRow={(record) => ({
              onClick: () => navigate(routes.settings.user(record.id)),
              style: { cursor: 'pointer' },
            })}
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
    </SettingsSection>
  );
}

const Muted = ({ children }: { children: React.ReactNode }) => (
  <span className={styles.muted}>{children}</span>
);
