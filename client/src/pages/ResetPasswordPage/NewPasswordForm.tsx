import { Button, Form, Input } from 'antd';
import { useNavigate } from 'react-router';
import { HttpError } from '../../api/core';
import { resetPassword } from '../../api/users';
import { PASSWORD_RULES } from '../../core/credentials';
import { useFormSubmit } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import { DEAD_LINKS, type DeadLink } from './deadLink';

interface NewPasswordFormProps {
  token: string;
  /** Raised when the server refuses the link, so the page can stop offering it. */
  onDeadLink: (dead: DeadLink) => void;
}

export function NewPasswordForm({ token, onDeadLink }: NewPasswordFormProps) {
  const [form] = Form.useForm<NewPasswordFormValues>();
  const { setMe } = useIdentity();
  const navigate = useNavigate();

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: async (values: NewPasswordFormValues) => {
      try {
        return await resetPassword({ token, password: values.password });
      } catch (err) {
        // A refused link isn't something this form can fix, so it replaces the
        // form instead of toasting over it.
        const dead =
          err instanceof HttpError ? DEAD_LINKS[err.code ?? ''] : undefined;
        if (dead) {
          onDeadLink(dead);
        }
        throw err;
      }
    },
    onSuccess: (me) => {
      setMe(me);
      navigate(routes.settings.authorization());
    },
    errorTitle: "Couldn't set your new password",
  });

  return (
    <Form<NewPasswordFormValues>
      form={form}
      layout="vertical"
      onFinish={onFinish}
      requiredMark={false}
      disabled={isSubmitting}
    >
      <Form.Item
        label="New password"
        name="password"
        rules={[{ required: true, message: 'Required' }, ...PASSWORD_RULES]}
      >
        <Input.Password autoComplete="new-password" autoFocus />
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
        Set the password and sign in
      </Button>
    </Form>
  );
}

interface NewPasswordFormValues {
  password: string;
  repeatPassword: string;
}
