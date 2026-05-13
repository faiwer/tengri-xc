import { PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import { Button, Input, Segmented, Skeleton, Tooltip, Tree } from 'antd';
import { useEffect, useMemo, useState } from 'react';

import { SportIo, type Sport } from '../../../api/admin/gliders.io';
import { LoadError } from '../../../components/LoadError';
import { useLocalStorageValue } from '../../../utils/useLocalStorageValue';
import { SettingsSection } from '../SettingsSection';
import { buildTree, type TreeBuild } from './buildTree';
import { useGliderCatalog } from './useGliderCatalog';
import styles from './GlidersSettings.module.scss';

/**
 * Admin: canonical brand + glider-model dictionary. Read-only for now —
 * the Add button is a placeholder for future write flows. Each sport is
 * fetched independently when its tab becomes active; the reload button
 * re-fetches the current sport.
 */
export function GlidersSettings() {
  const [sport, setSport] = useLocalStorageValue(
    'admin.gliders.sport',
    SPORT_STORAGE_OPTIONS,
  );
  const [query, setQuery] = useState('');
  const { data, isLoading, error, reload } = useGliderCatalog(sport);

  const { treeData, autoExpandedKeys } = useMemo<TreeBuild>(
    () =>
      data === null
        ? { treeData: [], autoExpandedKeys: [] }
        : buildTree(data, sport, query),
    [data, sport, query],
  );

  // `expandedKeys` follows search + sport by default, but users can override
  // by toggling nodes by hand. Reseat to the auto set whenever the search /
  // sport / data change.
  const [expandedKeys, setExpandedKeys] = useState<TreeKey[]>([]);
  useEffect(() => {
    setExpandedKeys(autoExpandedKeys);
  }, [autoExpandedKeys]);

  return (
    <SettingsSection
      title="Gliders"
      scrollable
      action={
        <div className={styles.actions}>
          <Tooltip title="Add brand or model — coming soon">
            <Button icon={<PlusOutlined />} disabled />
          </Tooltip>
          <Tooltip title="Reload">
            <Button
              icon={<ReloadOutlined />}
              onClick={reload}
              loading={isLoading}
            />
          </Tooltip>
          <Input.Search
            allowClear
            placeholder="Search brand or model"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className={styles.search}
          />
        </div>
      }
    >
      <div className={styles.controls}>
        <Segmented<Sport>
          options={SPORT_OPTIONS}
          value={sport}
          onChange={setSport}
        />
      </div>
      {error !== null && data === null ? (
        <LoadError
          title="Couldn't load gliders"
          error={error}
          onRetry={reload}
        />
      ) : data === null ? (
        <Skeleton active paragraph={{ rows: 6 }} />
      ) : treeData.length === 0 ? (
        <div className={styles.empty}>
          {query
            ? `No brands or models match "${query}".`
            : 'No models in the dictionary yet.'}
        </div>
      ) : (
        <Tree
          treeData={treeData}
          expandedKeys={expandedKeys}
          onExpand={setExpandedKeys}
          expandAction="click"
          selectable={false}
          showIcon
          showLine
          blockNode
        />
      )}
    </SettingsSection>
  );
}

/**
 * Antd `Tree` keys are `string | number`. React itself used to export
 * a `Key` alias for this; the fork we're on doesn't, so re-declare it
 * locally rather than pull from a non-public antd subpath.
 */
type TreeKey = string | number;

const SPORT_OPTIONS: { label: string; value: Sport }[] = [
  { label: 'PG', value: 'pg' },
  { label: 'HG', value: 'hg' },
  { label: 'SP', value: 'sp' },
];

const SPORT_STORAGE_OPTIONS = {
  schema: SportIo,
  defaultValue: 'pg' as Sport,
  strategy: 'initOnly' as const,
};
