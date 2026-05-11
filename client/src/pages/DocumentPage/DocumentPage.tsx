import { Skeleton } from 'antd';
import { useState } from 'react';
import { getSiteDocument } from '../../api/site';
import type { DocKind } from '../../api/site.io';
import { LoadError } from '../../components/LoadError';
import { Markdown } from '../../components/Markdown';
import { PageLayout } from '../../components/PageLayout';
import { useAsyncEffect, useEventHandler } from '../../core/hooks';
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
  const [error, setError] = useState<unknown>(null);
  const [retryToken, setRetryToken] = useState(0);

  useAsyncEffect(
    async (signal) => {
      setError(null);
      try {
        const next = await getSiteDocument(kind, { signal });
        if (!signal.aborted) setMd(next);
      } catch (err) {
        if (!signal.aborted) setError(err);
      }
    },
    [kind, retryToken],
  );

  const retry = useEventHandler(() => {
    setMd(undefined);
    setRetryToken((t) => t + 1);
  });

  return (
    <PageLayout fit>
      <article>
        <h1 className={styles.title}>{title}</h1>
        <DocumentBody md={md} error={error} onRetry={retry} title={title} />
      </article>
    </PageLayout>
  );
}

interface DocumentBodyProps {
  /** `undefined` = still loading, `null` = 404, string = published. */
  md: string | null | undefined;
  error: unknown;
  onRetry: () => void;
  title: string;
}

function DocumentBody({ md, error, onRetry, title }: DocumentBodyProps) {
  if (error && md === undefined) {
    return (
      <LoadError
        title={`Couldn't load ${title.toLowerCase()}`}
        error={error}
        onRetry={onRetry}
      />
    );
  }

  if (md === undefined) {
    return <Skeleton active paragraph={{ rows: 12 }} />;
  }

  if (md === null) {
    return <p className={styles.empty}>This page hasn't been published yet.</p>;
  }

  return <Markdown>{md}</Markdown>;
}
