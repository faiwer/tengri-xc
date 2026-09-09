import { Button, Form, Input, Typography } from 'antd';
import { useState } from 'react';
import { requestPasswordReset } from '../../api/users';
import { useFormSubmit } from '../../core/hooks';

/**
 * Asks for an address and has a reset link mailed to it. The server answers the
 * same way whether or not the address belongs to an account, so this shows the
 * same confirmation either way — anything else would turn the form into a way
 * to find out who is registered here.
 */
export function ResetPasswordForm({ onClose }: { onClose: () => void }) {
  const [form] = Form.useForm<ResetPasswordFormValues>();
  const [sentTo, setSentTo] = useState<string | null>(null);

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: async (values: ResetPasswordFormValues) => {
      const email = values.email.trim();
      await requestPasswordReset({ email });
      return email;
    },
    onSuccess: setSentTo,
    errorTitle: "Couldn't send the reset link",
  });

  if (sentTo) {
    return (
      <>
        <Typography.Paragraph>
          If <strong>{sentTo}</strong> belongs to an account, a link to choose a
          new password is on its way.
        </Typography.Paragraph>
        <Typography.Paragraph type="secondary">
          The link works for 24 hours and only once.
        </Typography.Paragraph>
        <Button type="primary" onClick={onClose} block>
          Got it
        </Button>
      </>
    );
  }

  return (
    <Form<ResetPasswordFormValues>
      form={form}
      layout="vertical"
      onFinish={onFinish}
      requiredMark={false}
      disabled={isSubmitting}
    >
      <Typography.Paragraph type="secondary">
        We'll email you a link to choose a new password.
      </Typography.Paragraph>

      <Form.Item
        label="Email"
        name="email"
        rules={[
          { required: true, message: 'Required' },
          { type: 'email', message: 'Enter a valid email address' },
        ]}
      >
        <Input autoComplete="email" autoFocus />
      </Form.Item>

      <Button type="primary" htmlType="submit" loading={isSubmitting} block>
        Send the link
      </Button>
    </Form>
  );
}

interface ResetPasswordFormValues {
  email: string;
}
