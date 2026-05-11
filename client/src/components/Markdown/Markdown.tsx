import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import styles from './Markdown.module.scss';

type MarkdownProps = {
  /** Raw markdown body. */
  children: string;
};

/**
 * Renders a GFM-flavoured markdown body with the app's shared typography
 * (headings, lists, code, tables, blockquotes, …). The wrapper `<div>` carries
 * the class so the styles cascade to whatever elements `react-markdown` emits
 * without per-element customisation.
 *
 * Default-exported so a dynamic `import('./Markdown')` reaches the component
 * directly — `lazyComponent(async () => (await import('./Markdown')) .default,
 * …)` is the call shape.
 */
export default function Markdown({ children }: MarkdownProps) {
  return (
    <div className={styles.markdown}>
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{children}</ReactMarkdown>
    </div>
  );
}
