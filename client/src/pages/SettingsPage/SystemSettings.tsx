import { Button, Form, Input, Skeleton, Switch } from 'antd';
import { useMemo, useState } from 'react';
import { Navigate } from 'react-router';
import { getAdminSite, updateAdminSite } from '../../api/admin/site';
import type { AdminSite } from '../../api/admin/site.io';
import { useAsyncEffect, useFormSubmit } from '../../core/hooks';
import { hasPermission, Permissions, useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import { useSite } from '../../core/site';
import { shallowEqual } from '../../utils/shallowEqual';
import { SettingsSection } from './SettingsSection';

/**
 * Operator editor at `/settings/system`. Two layers of gate before the form
 * mounts:
 *
 * 1. Identity still loading → skeleton; redirect anonymous viewers to `/login`;
 *    redirect non-MANAGE_SETTINGS viewers home so a URL-typed nav doesn't
 *    surface a 403 page.
 * 2. The admin payload fetches on mount via `GET /admin/site` — the public
 *    `useSite()` context intentionally doesn't carry the long-form markdown.
 */
export function SystemSettings() {
  const { me, isLoading } = useIdentity();

  if (isLoading) {
    return <Skeleton active paragraph={{ rows: 8 }} />;
  }

  if (!me) {
    return <Navigate replace to={routes.login()} />;
  }

  if (!hasPermission(me, Permissions.MANAGE_SETTINGS)) {
    return <Navigate replace to={routes.settings.profile()} />;
  }

  return <SystemSettingsLoader />;
}

function SystemSettingsLoader() {
  const [initial, setInitial] = useState<AdminSite | null>(null);

  useAsyncEffect(async (signal) => {
    const next = await getAdminSite({ signal });
    if (!signal.aborted) setInitial(next);
  }, []);

  if (!initial) {
    return <Skeleton active paragraph={{ rows: 10 }} />;
  }

  return <SystemSettingsForm initial={initial} />;
}

type SystemSettingsFormValues = {
  siteName: string;
  canRegister: boolean;
  tosMd: string;
  privacyMd: string;
};

interface SystemSettingsFormProps {
  initial: AdminSite;
}

function SystemSettingsForm({ initial }: SystemSettingsFormProps) {
  const [form] = Form.useForm<SystemSettingsFormValues>();
  const { setSite } = useSite();
  const [current, setCurrent] = useState(initial);

  const formInitial = useMemo<SystemSettingsFormValues>(
    () => toFormValues(current),
    [current],
  );

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values: SystemSettingsFormValues) =>
      updateAdminSite(fromFormValues(values)),
    onSuccess: (next) => {
      setCurrent(next);
      // Refresh the public site context so the header and footer
      // pick up the new branding / doc-link availability without a
      // page reload.
      setSite({
        siteName: next.siteName,
        canRegister: next.canRegister,
        hasTos: next.tosMd !== null,
        hasPrivacy: next.privacyMd !== null,
      });
      // Mirror server-normalised values back into the form (e.g. trimmed
      // `siteName`). `form.resetFields()` would rewind to the mount-time
      // `initialValues` snapshot — AntD doesn't pick up the recomputed
      // `formInitial` prop after mount — so the editor would visually revert to
      // the pre-save text.
      form.setFieldsValue(toFormValues(next));
    },
    successTitle: 'System settings saved',
    errorTitle: "Couldn't save system settings",
  });

  const values = Form.useWatch([], form) as
    | SystemSettingsFormValues
    | undefined;
  const isDirty = useMemo(
    () => !!values && !shallowEqual(values, formInitial),
    [values, formInitial],
  );

  return (
    <SettingsSection
      title="System settings"
      subtitle="Site-wide configuration."
      action={
        isDirty && (
          <Button
            type="primary"
            loading={isSubmitting}
            onClick={() => form.submit()}
          >
            Save
          </Button>
        )
      }
    >
      <Form<SystemSettingsFormValues>
        form={form}
        layout="vertical"
        initialValues={formInitial}
        onFinish={onFinish}
      >
        <Form.Item
          name="siteName"
          label={<span>Site name</span>}
          tooltip="Replaces 'Tengri XC' in the header and outgoing emails."
          rules={[{ required: true, message: 'Required' }]}
        >
          <Input maxLength={64} showCount />
        </Form.Item>
        <Form.Item
          name="canRegister"
          label={<span>Allow public registration</span>}
          tooltip="Off: only admins can create users. (Forward-looking — the public signup endpoint isn't built yet.)"
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
        <Form.Item
          name="tosMd"
          label={<span>Terms of Service (Markdown)</span>}
          tooltip="Rendered at /terms. Empty hides the footer link."
        >
          <Input.TextArea autoSize={{ minRows: 6, maxRows: 24 }} />
        </Form.Item>
        <Form.Item
          name="privacyMd"
          label={<span>Privacy Policy (Markdown)</span>}
          tooltip="Rendered at /privacy. Empty hides the footer link."
        >
          <Input.TextArea autoSize={{ minRows: 6, maxRows: 24 }} />
        </Form.Item>
      </Form>
    </SettingsSection>
  );
}

function toFormValues(site: AdminSite): SystemSettingsFormValues {
  return {
    siteName: site.siteName,
    canRegister: site.canRegister,
    // Form fields are non-null strings; `null` in the DB renders as
    // an empty textarea, and submitting empty round-trips back to
    // NULL via the server's "empty string = clear" rule.
    tosMd: site.tosMd ?? '',
    privacyMd: site.privacyMd ?? '',
  };
}

function fromFormValues(values: SystemSettingsFormValues) {
  return {
    siteName: values.siteName.trim(),
    canRegister: values.canRegister,
    tosMd: values.tosMd,
    privacyMd: values.privacyMd,
  };
}
