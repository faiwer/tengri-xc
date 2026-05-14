import { DatabaseOutlined, HomeOutlined } from '@ant-design/icons';
import { Tree } from 'antd';
import type { TreeProps } from 'antd';

import type { Sport } from '../../../api/admin/gliders.io';
import { GliderKindIcon } from '../../../components/icons';
import styles from './GlidersTree.module.scss';

/**
 * A wrapper around antd `Tree` tailored for the settings glider trees.
 */
export const GlidersTree = (props: TreeProps) => (
  <Tree
    className={styles.root}
    selectable={false}
    showIcon
    showLine
    blockNode
    {...props}
  />
);

export const BrandIcon = () => <HomeOutlined className={styles.brandIcon} />;

export const ClassIcon = () => (
  <DatabaseOutlined className={styles.classIcon} />
);

export const KindIcon = ({ kind }: { kind: Sport }) => (
  <GliderKindIcon kind={kind} className={styles.kindIcon} />
);
