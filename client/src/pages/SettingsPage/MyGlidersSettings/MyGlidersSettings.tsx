import { ReloadOutlined } from '@ant-design/icons';
import { Button, Skeleton, Tooltip } from 'antd';
import { useMemo } from 'react';

import { LoadError } from '../../../components/LoadError';
import { GlidersTree } from '../GlidersTree';
import { SettingsSection } from '../SettingsSection';
import { buildTree } from './buildTree';
import { useMyGliders } from './useMyGliders';
import styles from './MyGlidersSettings.module.scss';

/**
 * Owner-self glider list. Read-only: a wing comes into being when the pilot
 * flies it (via upload / the Leonardo importer), and goes away when the last
 * flight on it is deleted — there's no stand-alone create or delete affordance
 * here.
 *
 * Custom (pilot-private) wings show a `private` tag; rows where the importer
 * couldn't classify the wing carry an `unknown class` tag so it's visible in
 * the catalogue.
 */
export function MyGlidersSettings() {
  const { data, isLoading, error, reload } = useMyGliders();

  const { treeData, expandedKeys } = useMemo(
    () =>
      data === null ? { treeData: [], expandedKeys: [] } : buildTree(data),
    [data],
  );

  return (
    <SettingsSection
      title="Gliders"
      subtitle="Wings you've flown."
      action={
        <Tooltip title="Reload">
          <Button
            icon={<ReloadOutlined />}
            onClick={reload}
            loading={isLoading}
          />
        </Tooltip>
      }
    >
      {error !== null && data === null ? (
        <LoadError
          title="Couldn't load your gliders"
          error={error}
          onRetry={reload}
        />
      ) : data === null ? (
        <Skeleton active paragraph={{ rows: 6 }} />
      ) : data.length === 0 ? (
        <div className={styles.empty}>
          You don't have any gliders yet. They'll show up here as soon as you
          upload a flight.
        </div>
      ) : (
        <GlidersTree
          treeData={treeData}
          expandedKeys={expandedKeys}
          onExpand={() => undefined}
        />
      )}
    </SettingsSection>
  );
}
