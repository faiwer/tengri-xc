import { Alert, Button, Input, Skeleton, Table } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useMemo } from 'react';
import { Link } from 'react-router';

import type { UserListItem } from '../../api/admin/users.io';
import { useErrorToast } from '../../core/hooks';
import { routes } from '../../core/routes';
import { formatShortDate } from '../../utils/formatDateTime';
import { PermissionBadges } from './PermissionBadges';
import styles from './UsersSettings.module.scss';
import { useUsersFeed } from './useUsersFeed';

export function UsersSettings() {
  const feed = useUsersFeed();
  useErrorToast(feed.error, { title: "Couldn't load users" });

  const columns = useMemo<ColumnsType<UserListItem>>(
    () => [
      {
        title: 'Name',
        dataIndex: 'name',
        key: 'name',
        render: (_value, user) => (
          <Link to={routes.settings.user(user.id)}>{user.name}</Link>
        ),
      },
      {
        title: 'Login',
        dataIndex: 'login',
        key: 'login',
        render: (login: string | null) => login ?? <Muted>—</Muted>,
      },
      {
        title: 'Email',
        dataIndex: 'email',
        key: 'email',
        render: (email: string | null) => email ?? <Muted>—</Muted>,
      },
      {
        title: 'Permissions',
        dataIndex: 'permissions',
        key: 'permissions',
        render: (bits: number) => <PermissionBadges bits={bits} compact />,
      },
      {
        title: 'Joined',
        dataIndex: 'createdAt',
        key: 'createdAt',
        width: 110,
        render: (epoch: number) => formatShortDate(epoch),
      },
      {
        title: 'Last login',
        dataIndex: 'lastLoginAt',
        key: 'lastLoginAt',
        width: 110,
        render: (epoch: number | null) =>
          epoch === null ? <Muted>never</Muted> : formatShortDate(epoch),
      },
    ],
    [],
  );

  // Inline error only for the empty/initial state — otherwise the
  // toast handles it and we keep showing the rows we already have.
  const hasInlineError = feed.error !== null && feed.items === null;

  return (
    <section className={styles.section}>
      <header className={styles.header}>
        <h2 className={styles.title}>Users</h2>
        <Input.Search
          allowClear
          placeholder="Search by name or email"
          value={feed.query}
          onChange={(e) => feed.setQuery(e.target.value)}
          className={styles.search}
        />
      </header>

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
    </section>
  );
}

const Muted = ({ children }: { children: React.ReactNode }) => (
  <span className={styles.muted}>{children}</span>
);
