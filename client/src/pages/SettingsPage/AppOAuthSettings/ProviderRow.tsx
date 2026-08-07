import { Button, Form, Input, Switch } from 'antd';
import { useMemo, useState } from 'react';
import { updateAdminOAuthProvider } from '../../../api/admin/oauthProviders';
import type { AdminOAuthProvider } from '../../../api/admin/oauthProviders.io';
import { useFormSubmit } from '../../../core/hooks';
import { shallowEqual } from '../../../utils/shallowEqual';
import styles from './ProviderRow.module.scss';
import { TextWithIcon } from '../../../components/TextWithIcon';
import type { OAuthProviderMeta } from '../../../features/oauth/providers';

interface ProviderRowProps {
  provider: OAuthProviderMeta;
  /** The stored config, or `null` when this provider isn't set up yet. */
  config: AdminOAuthProvider | null;
}

/**
 * One provider's credential form. Self-contained: it seeds from `config` at
 * mount and, on save, re-seeds from its own entry in the refreshed list (the
 * PATCH returns the whole list). Sibling rows are untouched, so an in-progress
 * edit elsewhere survives this row's save.
 */
export function ProviderRow({ provider, config }: ProviderRowProps) {
  const [form] = Form.useForm<ProviderFormValues>();
  const [current, setCurrent] = useState(config);

  const formInitial = useMemo(() => toFormValues(current), [current]);

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values: ProviderFormValues) =>
      updateAdminOAuthProvider(provider.id, {
        clientId: values.clientId.trim(),
        clientSecret: values.clientSecret,
        enabled: values.enabled,
      }),
    onSuccess: (list) => {
      const next = list.find((c) => c.provider === provider.id) ?? null;
      setCurrent(next);
      form.setFieldsValue(toFormValues(next));
    },
    successTitle: `${provider.label} settings saved`,
    errorTitle: `Couldn't save ${provider.label} settings`,
  });

  const values = Form.useWatch<ProviderFormValues | null>([], form);
  const isDirty = useMemo(
    () => !!values && !shallowEqual(values, formInitial),
    [values, formInitial],
  );

  return (
    <div className={styles.provider}>
      <div className={styles.head}>
        <span className={styles.label}>
          <TextWithIcon icon={<provider.Icon />} text={provider.label} />
        </span>
        {isDirty && (
          <Button
            type="primary"
            size="small"
            loading={isSubmitting}
            onClick={() => form.submit()}
          >
            Save
          </Button>
        )}
      </div>
      <Form<ProviderFormValues>
        form={form}
        layout="vertical"
        initialValues={formInitial}
        onFinish={onFinish}
        className={styles.form}
      >
        <Form.Item
          name="clientId"
          label={<span>Client ID</span>}
          rules={[{ required: true, message: 'Required' }]}
        >
          <Input autoComplete="off" maxLength={FIELD_MAX_LEN} />
        </Form.Item>
        <Form.Item
          name="clientSecret"
          label={<span>Client secret</span>}
          rules={[{ required: true, message: 'Required' }]}
        >
          <Input.Password autoComplete="off" maxLength={FIELD_MAX_LEN} />
        </Form.Item>
        <Form.Item
          name="enabled"
          label={<span>Enabled</span>}
          tooltip="Off keeps the credentials stored but hides the provider from login."
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
      </Form>
    </div>
  );
}

const toFormValues = (
  config: AdminOAuthProvider | null,
): ProviderFormValues => ({
  clientId: config?.clientId ?? '',
  clientSecret: config?.clientSecret ?? '',
  enabled: config?.enabled ?? false,
});

type ProviderFormValues = {
  clientId: string;
  clientSecret: string;
  enabled: boolean;
};

const FIELD_MAX_LEN = 512;
