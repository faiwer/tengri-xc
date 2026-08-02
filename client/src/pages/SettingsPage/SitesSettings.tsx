import { Button, Input, Skeleton, Table } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useMemo } from 'react';

import type { SiteListItem } from '../../api/admin/sites.io';
import { Flag } from '../../components/Flag';
import { LoadError } from '../../components/LoadError';
import { useErrorToast } from '../../core/hooks';
import { SettingsSection } from './SettingsSection';
import styles from './SitesSettings.module.scss';
import { useSitesFeed } from './useSitesFeed';

export function SitesSettings() {
  const feed = useSitesFeed();
  useErrorToast(feed.error, { title: "Couldn't load sites" });

  const columns = useMemo<ColumnsType<SiteListItem>>(
    () => [
      { title: 'ID', dataIndex: 'id', key: 'id', width: '32px' },
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
        title: 'Lat',
        dataIndex: 'lat',
        key: 'lat',
        width: '120px',
        align: 'right',
        render: (lat: number) => lat.toFixed(5),
      },
      {
        title: 'Lng',
        dataIndex: 'lng',
        key: 'lng',
        width: '120px',
        align: 'right',
        render: (lng: number) => lng.toFixed(5),
      },
    ],
    [],
  );

  // Inline error only for the empty/initial state — otherwise the toast handles
  // it and we keep showing the rows we already have.
  const hasInlineError = !!feed.error && !feed.items?.length;

  return (
    <SettingsSection
      title="Sites"
      scrollable
      action={
        <Input.Search
          allowClear
          placeholder="Search by name"
          value={feed.query}
          onChange={(e) => feed.setQuery(e.target.value)}
          className={styles.search}
        />
      }
    >
      {hasInlineError ? (
        <LoadError
          title="Couldn't load sites"
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
                ? `No sites match "${feed.query}".`
                : 'No sites yet.',
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
