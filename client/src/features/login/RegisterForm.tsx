import { Button, Form, Input, Typography } from 'antd';
import { useState } from 'react';
import { register } from '../../api/users';
import { LOGIN_RULES, PASSWORD_RULES } from '../../core/credentials';
import { useFormSubmit } from '../../core/hooks';

export function RegisterForm({ onClose }: { onClose: () => void }) {
  const [form] = Form.useForm<RegisterFormValues>();
  const [sentTo, setSentTo] = useState<string | null>(null);

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: async (values: RegisterFormValues) => {
      const email = values.email.trim();
      await register({
        name: values.name.trim(),
        login: values.login.trim(),
        email,
        password: values.password,
      });
      return email;
    },
    onSuccess: setSentTo,
    errorTitle: "Couldn't create your account",
  });

  // Registration deliberately hands back no session, so closing the modal on
  // success would leave the user with nothing to go on.
  if (sentTo) {
    return (
      <>
        <Typography.Paragraph>
          We've sent a confirmation link to <strong>{sentTo}</strong>. Open it
          to finish setting up your account — following it signs you in.
        </Typography.Paragraph>
        <Typography.Paragraph type="secondary">
          The link works for 24 hours.
        </Typography.Paragraph>
        <Button type="primary" onClick={onClose} block>
          Got it
        </Button>
      </>
    );
  }

  return (
    <Form<RegisterFormValues>
      form={form}
      layout="vertical"
      onFinish={onFinish}
      requiredMark={false}
      disabled={isSubmitting}
    >
      <Form.Item
        label="Login"
        name="login"
        rules={[{ required: true, message: 'Required' }, ...LOGIN_RULES]}
      >
        <Input autoComplete="username" autoFocus />
      </Form.Item>

      <Form.Item
        label="Name"
        name="name"
        rules={[{ required: true, message: 'Required' }]}
      >
        <Input autoComplete="name" />
      </Form.Item>

      <Form.Item
        label="Email"
        name="email"
        rules={[
          { required: true, message: 'Required' },
          { type: 'email', message: 'Enter a valid email address' },
        ]}
      >
        <Input autoComplete="email" />
      </Form.Item>

      <Form.Item
        label="Password"
        name="password"
        rules={[{ required: true, message: 'Required' }, ...PASSWORD_RULES]}
      >
        <Input.Password autoComplete="new-password" />
      </Form.Item>

      <Form.Item
        label="Repeat your password"
        name="repeatPassword"
        dependencies={['password']}
        rules={[
          { required: true, message: 'Required' },
          ({ getFieldValue }) => ({
            validator(_, value) {
              if (!value || getFieldValue('password') === value) {
                return Promise.resolve();
              }
              return Promise.reject(new Error('Passwords do not match'));
            },
          }),
        ]}
      >
        <Input.Password autoComplete="new-password" />
      </Form.Item>

      <Button type="primary" htmlType="submit" loading={isSubmitting} block>
        Register
      </Button>
    </Form>
  );
}

interface RegisterFormValues {
  name: string;
  login: string;
  email: string;
  password: string;
  repeatPassword: string;
}
