import { HttpError } from '../../api/core';
import { Button, Form, Input } from 'antd';
import { login } from '../../api/users';
import { useAsync, useErrorToast } from '../../core/hooks';
import { useIdentity } from '../../core/identity';

export function SignInForm({ onClose }: { onClose: () => void }) {
  const { setMe } = useIdentity();

  const [submit, isLoading, error] = useAsync(
    async (values: LoginFormValues) => {
      setMe(await login(values));
      onClose();
    },
  );

  useErrorToast(loginErrorMessage(error) ?? error, {
    title: "Couldn't sign in",
  });

  return (
    <Form<LoginFormValues>
      layout="vertical"
      onFinish={submit}
      requiredMark={false}
      disabled={isLoading}
    >
      <Form.Item
        label="Login or email"
        name="identifier"
        rules={[{ required: true, message: 'Required' }]}
      >
        <Input autoComplete="username" autoFocus />
      </Form.Item>

      <Form.Item
        label="Password"
        name="password"
        rules={[{ required: true, message: 'Required' }]}
      >
        <Input.Password autoComplete="current-password" />
      </Form.Item>

      <Button type="primary" htmlType="submit" loading={isLoading} block>
        Sign in
      </Button>
    </Form>
  );
}

interface LoginFormValues {
  identifier: string;
  password: string;
}

const loginErrorMessage = (error: unknown): string | null => {
  if (!(error instanceof HttpError)) {
    return null;
  }

  if (error.status === 401) {
    return 'Wrong login or password';
  }

  // The password was right; the address on the account just isn't proven yet.
  // The server spells the reason out, so `message` is already the copy we want.
  if (error.code === 'email_unconfirmed') {
    return `Please check your email for a confirmation link to complete your registration.`;
  }

  return null;
};
