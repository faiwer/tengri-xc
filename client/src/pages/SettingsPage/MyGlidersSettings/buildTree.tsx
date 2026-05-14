import type { DataNode } from 'antd/es/tree';

import type { MyGlider } from '../../../api/me/gliders.io';
import type { Sport } from '../../../api/admin/gliders.io';
import { GliderKindIcon } from '../../../components/icons';
import { groupBy } from '../../../utils/groupBy';
import { BrandIcon, KindIcon } from '../GlidersTree';
import styles from './MyGlidersSettings.module.scss';

/**
 * Group a pilot's gliders into a `kind → brand → glider` tree. Leaves are
 * read-only: a flight is what creates the wing, so there's no stand-alone
 * delete here. Private (pilot-custom) and unknown-class rows get inline tags so
 * the operator/pilot sees what's curated vs. what's been auto-created from
 * imports.
 */
export function buildTree(gliders: MyGlider[]): TreeBuild {
  const treeData: DataNode[] = [];
  const expandedKeys: TreeKey[] = [];

  const byKind = groupBy(gliders, (g) => g.kind);

  for (const kind of KIND_ORDER) {
    const items = byKind.get(kind);
    if (!items || items.length === 0) {
      continue;
    }

    // Group by canonical brandId, then sort by the server-joined brand name so
    // the tree reads alphabetically per kind.
    const brands = [...groupBy(items, (g) => g.brandId).entries()]
      .map(([brandId, brandItems]) => ({
        brandId,
        label: brandItems[0].brandName,
        items: brandItems,
      }))
      .sort((a, b) => a.label.localeCompare(b.label));

    const brandNodes: DataNode[] = [];
    for (const { brandId, label, items: brandItems } of brands) {
      const brandNodeKey = `kind:${kind}:brand:${brandId}`;
      brandNodes.push({
        key: brandNodeKey,
        title: label,
        icon: <BrandIcon />,
        children: brandItems.map((g) => ({
          key: `glider:${g.kind}:${g.brandId}:${g.modelId}`,
          isLeaf: true,
          icon: <GliderKindIcon kind={g.kind} />,
          title: <Leaf glider={g} />,
        })),
      });
      expandedKeys.push(brandNodeKey);
    }

    const kindKey = `kind:${kind}`;
    treeData.push({
      key: kindKey,
      title: KIND_LABEL[kind],
      icon: <KindIcon kind={kind} />,
      children: brandNodes,
    });
    expandedKeys.push(kindKey);
  }

  return { treeData, expandedKeys };
}

function Leaf({ glider }: { glider: MyGlider }) {
  return (
    <div className={styles.leaf}>
      <span className={styles.leafLabel}>{glider.modelName}</span>
      {glider.isTandem && <span className={styles.tandem}>tandem</span>}
      {glider.class === 'unknown' && (
        <span
          className={styles.unknown}
          title="Class couldn't be inferred at import time."
        >
          unknown class
        </span>
      )}
      {glider.private && (
        <span
          className={styles.private}
          title="Custom wing — not in the canonical catalog."
        >
          private
        </span>
      )}
      <span className={styles.flightCount}>
        {glider.flightsCount} flight{glider.flightsCount === 1 ? '' : 's'}
      </span>
    </div>
  );
}

const KIND_ORDER: Sport[] = ['pg', 'hg', 'sp', 'other'];

const KIND_LABEL: Record<Sport, string> = {
  pg: 'Paragliders',
  hg: 'Hang-gliders',
  sp: 'Sailplanes',
  other: 'Other',
};

/** Whatever the antd Tree accepts as a node key. */
type TreeKey = string | number;

interface TreeBuild {
  treeData: DataNode[];
  /** Every kind + brand node, so the tree is fully expanded by default. The
   *  list is short (one pilot's gliders), so collapsing buys nothing. */
  expandedKeys: TreeKey[];
}
