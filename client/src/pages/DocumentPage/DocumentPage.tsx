import { Skeleton } from 'antd';
import { useState } from 'react';
import { getSiteDocument } from '../../api/site';
import type { DocKind } from '../../api/site.io';
import { Markdown } from '../../components/Markdown';
import { PageLayout } from '../../components/PageLayout';
import { useAsyncEffect } from '../../core/hooks';
import styles from './DocumentPage.module.scss';

interface DocumentPageProps {
  /** Which document to fetch. */
  kind: DocKind;
  /** Heading rendered above the markdown. */
  title: string;
}

/**
 * Static `/terms` and `/privacy` pages. Fetches the markdown body once on mount
 * via `getSiteDocument(kind)`. A `null` result (404 — column was NULL) renders
 * an "Not published" placeholder rather than failing, so a fresh install with
 * no docs uploaded shows something coherent if a visitor types the URL
 * directly.
 */
export function DocumentPage({ kind, title }: DocumentPageProps) {
  const [md, setMd] = useState<string | null | undefined>(undefined);

  useAsyncEffect(
    async (signal) => {
      const next = await getSiteDocument(kind, { signal });
      if (!signal.aborted) setMd(next);
    },
    [kind],
  );

  return (
    <PageLayout fit>
      <article>
        <h1 className={styles.title}>{title}</h1>
        <DocumentBody md={md} />
      </article>
    </PageLayout>
  );
}

interface DocumentBodyProps {
  /** `undefined` = still loading, `null` = 404, string = published. */
  md: string | null | undefined;
}

function DocumentBody({ md }: DocumentBodyProps) {
  if (md === undefined) {
    return <Skeleton active paragraph={{ rows: 12 }} />;
  }

  if (md === null) {
    return <p className={styles.empty}>This page hasn't been published yet.</p>;
  }

  return <Markdown>{md}</Markdown>;
}
