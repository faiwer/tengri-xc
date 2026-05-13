import { DatabaseOutlined, HomeOutlined } from '@ant-design/icons';
import type { DataNode } from 'antd/es/tree';

import type {
  GliderBrand,
  GliderCatalog,
  GliderClass,
  GliderModel,
  Sport,
} from '../../../api/admin/gliders.io';
import { GliderKindIcon } from '../../../components/icons';
import styles from './GlidersSettings.module.scss';

export interface TreeBuild {
  treeData: DataNode[];
  /** Brand + class nodes to open when there's a search query (so the matching
   *  leaves are visible). Empty when the query is blank. */
  autoExpandedKeys: string[];
}

/**
 * Group catalog models into a `brand → class → model` tree. Models are filtered
 * against `rawQuery` (matches brand name or model name); a brand-name hit keeps
 * all of the brand's models visible. `autoExpandedKeys` tells the consumer
 * which subtrees to open so the matching leaves are reachable without manual
 * expansion.
 */
export function buildTree(
  catalog: GliderCatalog,
  sport: Sport,
  rawQuery: string,
): TreeBuild {
  const query = rawQuery.trim().toLowerCase();
  const hasQuery = query.length > 0;

  const brandsById = new Map<string, GliderBrand>();
  for (const b of catalog.brands) {
    brandsById.set(b.id, b);
  }

  // Group models by (brandId, class), keeping the server's name order. When
  // searching, drop models whose own name and whose brand name both miss the
  // needle — a brand match keeps all of its children visible.
  const grouped = new Map<string, Map<GliderClass, GliderModel[]>>();
  for (const m of catalog.models) {
    if (hasQuery) {
      const brandHit = brandsById
        .get(m.brandId)
        ?.name.toLowerCase()
        .includes(query);
      const modelHit = m.name.toLowerCase().includes(query);
      if (!brandHit && !modelHit) {
        continue;
      }
    }

    let byClass = grouped.get(m.brandId);
    if (!byClass) {
      byClass = new Map();
      grouped.set(m.brandId, byClass);
    }

    let list = byClass.get(m.class);
    if (!list) {
      list = [];
      byClass.set(m.class, list);
    }

    list.push(m);
  }

  const order = CLASS_ORDER[sport];
  const modelIcon = <GliderKindIcon kind={sport} />;
  const treeData: DataNode[] = [];
  const autoExpandedKeys: string[] = [];

  for (const brand of catalog.brands) {
    const byClass = grouped.get(brand.id);
    if (!byClass) {
      continue;
    }

    const classNodes: DataNode[] = [];
    for (const cls of order) {
      const models = byClass.get(cls);
      if (!models || models.length === 0) {
        continue;
      }

      const classKey = `class:${brand.id}:${cls}`;
      classNodes.push({
        key: classKey,
        title: CLASS_LABEL[cls],
        icon: CLASS_ICON,
        children: models.map((m) => ({
          key: `model:${brand.id}:${m.id}`,
          title: m.isTandem ? (
            <>
              {m.name}
              <span className={styles.tandem}>tandem</span>
            </>
          ) : (
            m.name
          ),
          icon: modelIcon,
          isLeaf: true,
        })),
      });
      if (hasQuery) {
        autoExpandedKeys.push(classKey);
      }
    }

    if (classNodes.length === 0) {
      continue;
    }

    const brandKey = `brand:${brand.id}`;
    treeData.push({
      key: brandKey,
      title: brand.name,
      icon: BRAND_ICON,
      children: classNodes,
    });
    if (hasQuery) {
      autoExpandedKeys.push(brandKey);
    }
  }

  return { treeData, autoExpandedKeys };
}

/**
 * Display order for class buckets within a brand. Matches the natural
 * progression (entry → top tier for PG / HG; FAI comp classes for SP) so the
 * tree reads top-to-bottom the way a pilot would expect. `other` has no class
 * taxonomy and isn't picker-reachable from this page; mapped to an empty list
 * to satisfy the `Sport` record.
 */
const CLASS_ORDER: Record<Sport, readonly GliderClass[]> = {
  pg: ['en_a', 'en_b', 'en_c', 'en_d', 'ccc'],
  hg: ['single_surface', 'kingpost', 'topless', 'rigid'],
  sp: [
    'club',
    'standard',
    'fifteen_metre',
    'eighteen_metre',
    'twenty_metre_two_seater',
    'open',
    'motorglider',
  ],
  other: [],
};

const CLASS_LABEL: Record<GliderClass, string> = {
  en_a: 'EN A',
  en_b: 'EN B',
  en_c: 'EN C',
  en_d: 'EN D',
  ccc: 'CCC',
  single_surface: 'Single surface',
  kingpost: 'Kingpost',
  topless: 'Topless',
  rigid: 'Rigid',
  standard: 'Standard',
  fifteen_metre: '15 m',
  eighteen_metre: '18 m',
  twenty_metre_two_seater: '20 m two-seater',
  open: 'Open',
  club: 'Club',
  motorglider: 'Motorglider',
};

const BRAND_ICON = <HomeOutlined className={styles.brandIcon} />;
const CLASS_ICON = <DatabaseOutlined className={styles.classIcon} />;
