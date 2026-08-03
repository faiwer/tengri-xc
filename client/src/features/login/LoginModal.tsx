import { Button, Form, Input, Modal } from 'antd';
import { HttpError } from '../../api/core';
import { login } from '../../api/users';
import { useAsync, useErrorToast } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import styles from './LoginModal.module.scss';

interface LoginModalProps {
  open: boolean;
  onClose: () => void;
}

interface LoginFormValues {
  identifier: string;
  password: string;
}

/**
 * Username-or-email + password modal. On success: store the `Me` returned by
 * the server in the identity context and close. The current page re-renders
 * with the new identity; the session cookie is set by the server (HttpOnly).
 */
export function LoginModal({ open, onClose }: LoginModalProps) {
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
    <Modal
      title="Sign in"
      open={open}
      footer={null}
      width={400}
      onCancel={isLoading ? undefined : onClose}
      className={styles.modal}
    >
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
    </Modal>
  );
}

const loginErrorMessage = (error: unknown): string | null => {
  if (error instanceof HttpError && error.status === 401) {
    return 'Wrong login or password';
  }
  return null;
};
