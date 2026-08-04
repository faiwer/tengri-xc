import { Button, Form, Input, Skeleton } from 'antd';
import { Navigate } from 'react-router';
import { changeMyPassword } from '../../../api/users';
import type { Me } from '../../../api/users.io';
import { LoadError } from '../../../components/LoadError';
import { useFormSubmit } from '../../../core/hooks';
import { useIdentity } from '../../../core/identity';
import { routes } from '../../../core/routes';
import { SettingsSection } from '../SettingsSection';

export function AuthorizationSettings() {
  const { me, isLoading, error, retry, setMe } = useIdentity();

  if (isLoading) {
    return <Skeleton active paragraph={{ rows: 4 }} />;
  } else if (error) {
    return (
      <LoadError
        title="Couldn't load your account"
        error={error}
        onRetry={retry}
      />
    );
  } else if (!me) {
    return <Navigate replace to={routes.login()} />;
  }

  return <PasswordForm me={me} onSaved={setMe} />;
}

interface PasswordFormProps {
  me: Me;
  onSaved: (me: Me) => void;
}

interface PasswordFormValues extends Record<string, unknown> {
  login: string;
  currentPassword: string;
  newPassword: string;
  repeatPassword: string;
}

function PasswordForm({ me, onSaved }: PasswordFormProps) {
  const [form] = Form.useForm<PasswordFormValues>();

  // Both flags are independent: an account can have a login but no password
  // (import) or a password but no login (email-only OAuth).
  const loginEditable = me.login == null;
  const { hasPassword } = me;

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values) =>
      changeMyPassword({
        newPassword: values.newPassword,
        login: loginEditable ? values.login.trim() : undefined,
        currentPassword: hasPassword ? values.currentPassword : undefined,
      }),
    onSuccess: (updated) => {
      onSaved(updated);
      form.resetFields(['currentPassword', 'newPassword', 'repeatPassword']);
    },
    successTitle: 'Password saved',
    errorTitle: "Couldn't save password",
  });

  return (
    <SettingsSection
      title="Authorization"
      subtitle="Set or change the password you use to sign in."
      action={
        <Button
          type="primary"
          loading={isSubmitting}
          onClick={() => form.submit()}
        >
          Save
        </Button>
      }
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ flex: '10rem' }}
        labelAlign="left"
        wrapperCol={{ flex: '1 1 auto' }}
        initialValues={{
          login: me.login ?? '',
          currentPassword: '',
          newPassword: '',
          repeatPassword: '',
        }}
        onFinish={onFinish}
      >
        <Form.Item
          name="login"
          label="Login"
          rules={
            loginEditable
              ? [{ required: true, message: 'Choose a login' }]
              : undefined
          }
        >
          <Input
            placeholder={loginEditable ? 'Choose a login' : undefined}
            disabled={!loginEditable}
            autoComplete="username"
          />
        </Form.Item>

        {hasPassword && (
          <Form.Item
            name="currentPassword"
            label="Current password"
            rules={[{ required: true, message: 'Enter your current password' }]}
          >
            <Input.Password autoComplete="current-password" />
          </Form.Item>
        )}

        <Form.Item
          name="newPassword"
          label="New password"
          rules={[
            { required: true, message: 'Enter a new password' },
            { min: 8, message: 'At least 8 characters' },
            {
              pattern: PASSWORD_PATTERN,
              message: 'Must include a letter and a digit',
            },
          ]}
        >
          <Input.Password autoComplete="new-password" />
        </Form.Item>

        <Form.Item
          name="repeatPassword"
          label="Repeat password"
          dependencies={['newPassword']}
          rules={[
            { required: true, message: 'Repeat the new password' },
            ({ getFieldValue }) => ({
              validator(_, value) {
                if (!value || getFieldValue('newPassword') === value) {
                  return Promise.resolve();
                }
                return Promise.reject(new Error('Passwords do not match'));
              },
            }),
          ]}
        >
          <Input.Password autoComplete="new-password" />
        </Form.Item>
      </Form>
    </SettingsSection>
  );
}

// At least one letter and one digit anywhere; length is enforced by a separate
// `min` rule. Mirrors the server's `weak_password` check.
const PASSWORD_PATTERN = /^(?=.*[A-Za-z])(?=.*\d).+$/;
