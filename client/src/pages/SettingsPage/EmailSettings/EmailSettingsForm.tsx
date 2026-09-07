import { Button, Form } from 'antd';
import { useMemo, useState } from 'react';
import { updateAdminSite } from '../../../api/admin/site';
import type { AdminSite } from '../../../api/admin/site.io';
import { useFormSubmit } from '../../../core/hooks';
import { shallowEqual } from '../../../utils/shallowEqual';
import { SettingsSection } from '../SettingsSection';
import { ConnectionFields } from './ConnectionFields';
import { TemplateFields } from './TemplateFields';
import { TestEmailModal } from './TestEmailModal';
import styles from './EmailSettingsForm.module.scss';
import {
  type EmailSettingsFormValues,
  fromFormValues,
  toFormValues,
} from './formValues';

interface EmailSettingsFormProps {
  initial: AdminSite;
}

export function EmailSettingsForm({ initial }: EmailSettingsFormProps) {
  const [form] = Form.useForm<EmailSettingsFormValues>();
  const [current, setCurrent] = useState(initial);
  const [isTesting, setIsTesting] = useState(false);

  const formInitial = useMemo<EmailSettingsFormValues>(
    () => toFormValues(current),
    [current],
  );

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values: EmailSettingsFormValues) =>
      updateAdminSite(fromFormValues(values)),
    onSuccess: (next) => {
      setCurrent(next);
      // Mirror server-normalised values back (lowercased from-address, trimmed
      // host) and re-blank the password box. `form.resetFields()` would rewind
      // to the mount-time snapshot instead.
      form.setFieldsValue(toFormValues(next));
    },
    successTitle: 'Email settings saved',
    errorTitle: "Couldn't save email settings",
  });

  const values = Form.useWatch([], form) as EmailSettingsFormValues | undefined;
  const isDirty = useMemo(
    () => !!values && !shallowEqual(values, formInitial),
    [values, formInitial],
  );

  return (
    <SettingsSection
      title="Email"
      subtitle="Outgoing mail — password resets, verification links, notifications."
      scrollable
      action={
        <div className={styles.actions}>
          <Button onClick={() => setIsTesting(true)}>Send a test email</Button>
          {isDirty && (
            <Button
              type="primary"
              loading={isSubmitting}
              onClick={() => form.submit()}
            >
              Save
            </Button>
          )}
        </div>
      }
    >
      <Form<EmailSettingsFormValues>
        form={form}
        layout="vertical"
        initialValues={formInitial}
        onFinish={onFinish}
      >
        <ConnectionFields hasPassword={current.smtpPassword !== null} />
        <TemplateFields />
      </Form>
      <TestEmailModal
        open={isTesting}
        // `useWatch` hasn't reported yet on the first render.
        values={values ?? formInitial}
        onClose={() => setIsTesting(false)}
      />
    </SettingsSection>
  );
}
