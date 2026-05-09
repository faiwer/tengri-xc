import { Button, Form, Input } from 'antd';
import { Navigate, useNavigate } from 'react-router';
import { HttpError } from '../../api/core';
import { login } from '../../api/users';
import { PageLayout } from '../../components/PageLayout';
import { useAsync, useErrorToast } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import styles from './LoginPage.module.scss';

interface LoginFormValues {
  identifier: string;
  password: string;
}

/**
 * Minimal username-or-email + password form. On success: store the
 * `Me` returned by the server in the identity context, navigate to
 * `/flights`. The session cookie is set by the server (HttpOnly).
 */
export function LoginPage() {
  const { me, setMe } = useIdentity();
  const navigate = useNavigate();

  const [submit, isLoading, error] = useAsync(
    async (values: LoginFormValues) => {
      const next = await login(values);
      setMe(next);
      navigate(routes.flights());
    },
  );

  useErrorToast(loginErrorMessage(error) ?? error, {
    title: "Couldn't sign in",
  });

  if (me) {
    return <Navigate to={routes.flights()} replace />;
  }

  return (
    <PageLayout>
      <Form<LoginFormValues>
        layout="vertical"
        className={styles.card}
        onFinish={submit}
        requiredMark={false}
        disabled={isLoading}
      >
        <h1 className={styles.title}>Sign in</h1>

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
    </PageLayout>
  );
}

const loginErrorMessage = (error: unknown): string | null => {
  if (error instanceof HttpError && error.status === 401) {
    return 'Wrong login or password';
  }
  return null;
};
