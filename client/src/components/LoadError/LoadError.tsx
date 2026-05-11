import { Alert, Button } from 'antd';
import type { ReactNode } from 'react';
import styles from './LoadError.module.scss';

export interface LoadErrorProps {
  /** Bold heading shown on the alert. */
  title: string;
  /**
   * The thrown value from the failed load. Coerced to a sensible description
   * string (Error → message, anything else → String()). Pass a `string`
   * directly when the source already has a friendly sentence.
   */
  error: unknown;
  /**
   * If provided, render a primary "Retry" button that calls this. The label
   * says "Retry" rather than "Reload" because callers should be wiring a
   * *local* retry (re-issue the failed request, keep the rest of the page
   * mounted) — `window.location.reload()` is a fallback for places that
   * genuinely have no other way back.
   */
  onRetry?: () => void;
  /** Slot for extra trailing buttons next to Retry (e.g. "Back to users"). */
  extraActions?: ReactNode;
}

/**
 * Inline "couldn't load X" panel. The standard shape across the app — any page
 * whose initial fetch can fail should render one of these in place of the
 * skeleton instead of letting the skeleton spin forever.
 *
 * Keep the title scoped to the operation ("Couldn't load preferences") rather
 * than the page; the same component is also used for child-area failures inside
 * an otherwise-loaded page.
 */
export function LoadError({
  title,
  error,
  onRetry,
  extraActions,
}: LoadErrorProps) {
  const description =
    typeof error === 'string'
      ? error
      : error instanceof Error
        ? error.message
        : error
          ? String(error)
          : undefined;

  return (
    <Alert
      type="error"
      showIcon
      title={title}
      description={description}
      action={
        (onRetry || extraActions) && (
          <div className={styles.actions}>
            {onRetry && (
              <Button size="small" type="primary" onClick={onRetry}>
                Retry
              </Button>
            )}
            {extraActions}
          </div>
        )
      }
    />
  );
}
