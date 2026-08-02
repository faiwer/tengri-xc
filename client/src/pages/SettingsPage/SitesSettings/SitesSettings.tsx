import { PlusOutlined, ThunderboltOutlined } from '@ant-design/icons';
import { App, Button, Input, Skeleton, Table, Tooltip } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useMemo, useState } from 'react';

import { deleteSite, reindexSites } from '../../../api/admin/sites';
import type { SiteListItem } from '../../../api/admin/sites.io';
import { Flag } from '../../../components/Flag';
import { LoadError } from '../../../components/LoadError';
import { useErrorToast, useEventHandler } from '../../../core/hooks';
import { SettingsSection } from '../SettingsSection';
import { SiteFormModal } from './SiteFormModal';
import { SiteRowActions } from './SiteRowActions';
import styles from './SitesSettings.module.scss';
import { useSitesFeed } from './useSitesFeed';

export function SitesSettings() {
  const feed = useSitesFeed();
  useErrorToast(feed.error, { title: "Couldn't load sites" });
  const { notification } = App.useApp();

  const [modalOpen, setModalOpen] = useState(false);
  // The row being edited, or `null` when the modal is opened to create.
  const [editing, setEditing] = useState<SiteListItem | null>(null);
  // Id of the row whose deletion is in flight, for its confirm-button spinner.
  const [deletingId, setDeletingId] = useState<number | null>(null);
  const [reindexing, setReindexing] = useState(false);

  const openCreate = () => {
    setEditing(null);
    setModalOpen(true);
  };

  const openEdit = useEventHandler((site: SiteListItem) => {
    setEditing(site);
    setModalOpen(true);
  });

  const onSaved = (site: SiteListItem) => {
    feed.onSaved(site);
    setModalOpen(false);
  };

  const onDelete = useEventHandler(async (site: SiteListItem) => {
    setDeletingId(site.id);
    try {
      await deleteSite(site.id);
      feed.onRemoved(site.id);
      notification.success({ title: 'Site deleted', placement: 'bottomRight' });
    } catch (err) {
      notification.error({
        title: "Couldn't delete site",
        description: err instanceof Error ? err.message : String(err),
        placement: 'bottomRight',
      });
    } finally {
      setDeletingId(null);
    }
  });

  const onReindex = async () => {
    setReindexing(true);
    try {
      const { updated } = await reindexSites();
      notification.success({
        title: `Reindexed ${updated} flights`,
        placement: 'bottomRight',
      });
    } catch (err) {
      notification.error({
        title: "Couldn't reindex flights",
        description: err instanceof Error ? err.message : String(err),
        placement: 'bottomRight',
      });
    } finally {
      setReindexing(false);
    }
  };

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
      {
        key: 'actions',
        width: '80px',
        align: 'center',
        render: (_, record) => (
          <SiteRowActions
            site={record}
            deleting={deletingId === record.id}
            onEdit={openEdit}
            onDelete={onDelete}
          />
        ),
      },
    ],
    [deletingId, openEdit, onDelete],
  );

  // Inline error only for the empty/initial state — otherwise the toast handles
  // it and we keep showing the rows we already have.
  const hasInlineError = !!feed.error && !feed.items?.length;

  return (
    <SettingsSection
      title="Sites"
      scrollable
      action={
        <div className={styles.actions}>
          <Input.Search
            allowClear
            placeholder="Search by name"
            value={feed.query}
            onChange={(e) => feed.setQuery(e.target.value)}
            className={styles.search}
          />
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={openCreate}
            aria-label="Add site"
            className={styles.actionBtn}
          />
          <Tooltip title="Reindex existing flights">
            <Button
              icon={<ThunderboltOutlined />}
              loading={reindexing}
              onClick={onReindex}
              aria-label="Reindex existing flights"
              className={styles.actionBtn}
            />
          </Tooltip>
        </div>
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

      <SiteFormModal
        open={modalOpen}
        site={editing}
        onSaved={onSaved}
        onClose={() => setModalOpen(false)}
      />
    </SettingsSection>
  );
}
