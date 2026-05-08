import { App } from 'antd';
import { useEffect } from 'react';

interface UseErrorToastOptions {
  /** Heading shown in bold on top of the toast. Default: a generic apology. */
  title?: string;
  /**
   * Body text. Defaults to a string-coerced view of `error` so the
   * caller usually doesn't have to think about it; pass a domain-aware
   * sentence when the raw error isn't user-friendly.
   */
  description?: string;
}

const DEFAULT_TITLE = 'Oops. Something went wrong';

/**
 * Pop a bottom-right error toast whenever `error` transitions from
 * nullish to a value. Subsequent renders with the same error don't
 * re-fire — only a fresh error (or a clear-then-set cycle) does.
 *
 * Pass `null` / `undefined` when nothing's wrong; the hook does
 * nothing in that case, which makes it safe to call unconditionally
 * from a component body that may or may not be in an error state.
 */
export function useErrorToast(
  error: string | null | undefined,
  options: UseErrorToastOptions = {},
): void {
  const { notification } = App.useApp();
  const { title = DEFAULT_TITLE, description } = options;

  useEffect(() => {
    if (error === null || error === undefined) {
      return;
    }

    notification.error({
      message: title,
      description: description ?? error,
      placement: 'bottomRight',
    });
  }, [error]);
}
