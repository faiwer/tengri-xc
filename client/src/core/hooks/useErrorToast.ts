import { App } from 'antd';
import { useAsyncEffect } from './useAsyncEffect';

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
 * Accepts whatever `useAsync` / a `catch` block hands you (`unknown`):
 * `Error` instances render as their `.message`, primitives string-coerce,
 * `null` / `undefined` are no-ops. Domain-specific rewrites (e.g. mapping
 * a 401 to "Wrong password") still belong at the call site — pass the
 * mapped string in directly.
 */
export function useErrorToast(
  error: unknown,
  options: UseErrorToastOptions = {},
): void {
  const { notification } = App.useApp();
  const { title = DEFAULT_TITLE, description } = options;

  useAsyncEffect(() => {
    const message =
      error instanceof Error ? error.message : error ? String(error) : null;
    if (!message) {
      return;
    }

    notification.error({
      message: title,
      description: description ?? message,
      placement: 'bottomRight',
    });
  }, [error]);
}
