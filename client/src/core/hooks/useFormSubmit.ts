import { App, type FormInstance } from 'antd';
import { useState } from 'react';
import { ValidationError } from '../../api/core';

interface UseFormSubmitOptions<TValues, TResult> {
  /** AntD form instance the submit is bound to. */
  form: FormInstance<TValues>;
  /** Async work that turns form values into a server response. */
  submit: (values: TValues) => Promise<TResult>;
  /** Called with the resolved result on success. */
  onSuccess?: (result: TResult) => void;
  /**
   * Toast title for non-validation errors. Validation errors land on
   * the form fields directly and never toast.
   */
  errorTitle?: string;
  /**
   * If the server's field paths are namespaced (e.g. `'preferences.units'`)
   * but the form uses the leaf names directly (`'units'`), pass the
   * prefix to strip. Without it, the names won't match and the error
   * never reaches the right field.
   */
  fieldPrefix?: string;
  /** Toast title for the success notification, if any. */
  successTitle?: string;
}

interface UseFormSubmitResult<TValues> {
  /** Pass directly to AntD `<Form onFinish={…} />`. */
  onFinish: (values: TValues) => Promise<void>;
  /** True while the submit promise is pending. Wire to the Save button. */
  isSubmitting: boolean;
}

/**
 * Wraps an AntD form `onFinish` handler with the standard pipeline:
 * track pending state, route 422 {@link ValidationError}s onto the form's
 * fields, toast everything else.
 *
 * The submit promise's *result* (typically the updated server payload)
 * is handed to `onSuccess` for the caller to commit elsewhere — e.g.
 * swapping a fresh `Me` into the identity context after a save.
 */
export function useFormSubmit<TValues, TResult>(
  options: UseFormSubmitOptions<TValues, TResult>,
): UseFormSubmitResult<TValues> {
  const { form, submit, onSuccess, errorTitle, fieldPrefix, successTitle } =
    options;
  const { notification } = App.useApp();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const onFinish = async (values: TValues): Promise<void> => {
    setIsSubmitting(true);
    try {
      const result = await submit(values);
      onSuccess?.(result);
      if (successTitle) {
        notification.success({
          message: successTitle,
          placement: 'bottomRight',
        });
      }
    } catch (err) {
      if (err instanceof ValidationError) {
        const fields = mapServerFieldsToFormFields(err.fields, fieldPrefix);
        if (fields.length > 0) {
          // AntD `setFields` is generic over the form values shape and
          // wants `name` typed against the path type for `TValues`. The
          // server gives us untyped strings; cast at the boundary so
          // call sites don't have to thread the form generic into the
          // error map.
          form.setFields(fields as Parameters<typeof form.setFields>[0]);
          return;
        }
      }

      notification.error({
        message: errorTitle ?? "Couldn't save",
        description: err instanceof Error ? err.message : String(err),
        placement: 'bottomRight',
      });
    } finally {
      setIsSubmitting(false);
    }
  };

  return { onFinish, isSubmitting };
}

interface FormFieldError {
  name: (string | number)[];
  errors: string[];
}

/**
 * Translate the server's flat dotted field paths (e.g.
 * `'preferences.units'`) into AntD `setFields` entries (e.g.
 * `{ name: ['units'], errors: ['…'] }`). Strips `fieldPrefix.` from
 * each key so the namespace from a multi-section endpoint doesn't
 * leak into a section-only form.
 */
function mapServerFieldsToFormFields(
  serverFields: Record<string, string>,
  prefix?: string,
): FormFieldError[] {
  const stripPrefix = prefix ? `${prefix}.` : '';
  const out: FormFieldError[] = [];
  for (const [path, message] of Object.entries(serverFields)) {
    const local =
      stripPrefix && path.startsWith(stripPrefix)
        ? path.slice(stripPrefix.length)
        : path;
    out.push({ name: local.split('.'), errors: [message] });
  }
  return out;
}
