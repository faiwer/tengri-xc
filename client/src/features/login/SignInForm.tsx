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

  useErrorToast(error, {
    title: "Couldn't sign in",
    description: loginErrorMessage(error) ?? undefined,
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

  // Both codes below mean the password was right and something else refused,
  // so each gets copy that says what to do about it. A 401 can't be told apart
  // from any other bad credentials, by design.
  if (error.code === 'account_disabled') {
    return `This account is disabled. Contact the site administrator if you think that's a mistake.`;
  }

  // The address on the account just isn't proven yet.
  if (error.code === 'email_unconfirmed') {
    return 'Please check your email for a confirmation link to complete your registration.';
  }

  if (error.status === 401) {
    return 'Wrong login or password';
  }

  return null;
};
