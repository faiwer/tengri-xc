import { Button, Form, Input, Modal } from 'antd';
import { sendTestEmail } from '../../../api/admin/site';
import { useFormSubmit } from '../../../core/hooks';
import { useIdentity } from '../../../core/identity';
import { type EmailSettingsFormValues } from './formValues';
import styles from './TestEmailModal.module.scss';

interface TestEmailModalProps {
  open: boolean;
  /**
   * The editor's live values. Passed through to the server so an unsaved
   * connection can be tried without saving it first.
   */
  values: EmailSettingsFormValues;
  onClose: () => void;
}

export function TestEmailModal({ open, values, onClose }: TestEmailModalProps) {
  return (
    <Modal
      title="Send a test email"
      open={open}
      footer={null}
      // Remount so the recipient box re-reads its default on each open instead
      // of keeping the previous attempt's text.
      destroyOnHidden
      onCancel={onClose}
    >
      <TestEmailForm values={values} onClose={onClose} />
    </Modal>
  );
}

interface TestEmailFormValues {
  to: string;
}

function TestEmailForm({ values, onClose }: Omit<TestEmailModalProps, 'open'>) {
  const { me } = useIdentity();
  const [form] = Form.useForm<TestEmailFormValues>();

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: ({ to }: TestEmailFormValues) => sendTestEmail({ ...values, to }),
    onSuccess: onClose,
    successTitle: 'Test email sent',
    errorTitle: "Couldn't send the test email",
  });

  return (
    <Form
      form={form}
      layout="vertical"
      initialValues={{ to: me?.email ?? '' }}
      onFinish={onFinish}
    >
      <Form.Item
        name="to"
        label={<span>Send to</span>}
        tooltip="Uses the settings currently in the form, including unsaved ones."
        rules={[{ required: true, message: 'Enter an email address' }]}
      >
        <Input type="email" autoComplete="off" placeholder="you@example.com" />
      </Form.Item>
      <div className={styles.actions}>
        <Button onClick={onClose} disabled={isSubmitting}>
          Cancel
        </Button>
        <Button
          type="primary"
          loading={isSubmitting}
          onClick={() => form.submit()}
        >
          Send
        </Button>
      </div>
    </Form>
  );
}
