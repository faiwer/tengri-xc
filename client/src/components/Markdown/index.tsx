import { Skeleton } from 'antd';
import { lazyComponent } from '../../utils/lazyComponent';

/**
 * Lazy-loaded markdown renderer. The actual component (plus `react-markdown`
 * and `remark-gfm`, ~100 KB of unified/remark/micromark machinery) lives in its
 * own chunk and only ships when something mounts `<Markdown>` for the first
 * time.
 *
 * Callers shouldn't think about that — the export here is the public `Markdown`
 * component. The default skeleton fits a doc-page-sized placeholder; override
 * per call site via the `lazyFallback` prop when the surrounding layout wants
 * something different (inline spinner, empty space, custom shimmer).
 */
export const Markdown = lazyComponent(
  async () => (await import('./Markdown')).default,
  <Skeleton active paragraph={{ rows: 12 }} />,
  'markdown',
);
