import { Button, Checkbox, Form, Input, InputNumber, Select } from 'antd';
import { useMemo } from 'react';

import { createUser, updateUser } from '../../../../api/admin/users';
import type { User, UserInput } from '../../../../api/admin/users.io';
import type { UserSex } from '../../../../api/users.io';
import { CountrySelect } from '../../../../components/CountrySelect';
import { useFormSubmit } from '../../../../core/hooks';
import { Permissions } from '../../../../core/identity';
import { SEX_OPTIONS } from '../../../../core/sex';
import styles from './UserForm.module.scss';

interface UserFormProps {
  /** The user being edited, or `null` to create a new one. */
  user: User | null;
  onSaved: (user: User) => void;
  onCancel: () => void;
}

export function UserForm({ user, onSaved, onCancel }: UserFormProps) {
  const [form] = Form.useForm<UserFormValues>();
  const initial = useMemo(() => formInitial(user), [user]);

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values) => {
      const input = normalize(values);
      return user ? updateUser(user.id, input) : createUser(input);
    },
    onSuccess: onSaved,
    successTitle: user ? 'User saved' : 'User created',
    errorTitle: "Couldn't save user",
  });

  return (
    <Form
      form={form}
      layout="horizontal"
      labelCol={{ flex: '7.5rem' }}
      labelAlign="left"
      wrapperCol={{ flex: '1 1 auto' }}
      initialValues={initial}
      onFinish={onFinish}
    >
      <Form.Item
        name="name"
        label="Name"
        rules={[{ required: true, message: 'Enter a name' }]}
      >
        <Input placeholder="Display name" />
      </Form.Item>

      <Form.Item name="login" label="Login">
        <Input placeholder="Username" autoComplete="off" />
      </Form.Item>

      <Form.Item name="email" label="Email">
        <Input
          type="email"
          placeholder="pilot@example.com"
          autoComplete="off"
        />
      </Form.Item>

      <Form.Item
        name="emailVerified"
        label="Email verified"
        valuePropName="checked"
      >
        <Checkbox>Address is verified</Checkbox>
      </Form.Item>

      <Form.Item name="permissions" label="Permissions">
        <Checkbox.Group
          options={PERMISSION_OPTIONS}
          className={styles.permissions}
        />
      </Form.Item>

      <Form.Item name="password" label="Password">
        <Input.Password
          autoComplete="new-password"
          placeholder={user ? 'Leave blank to keep current' : 'Set a password'}
        />
      </Form.Item>

      <Form.Item name={['profile', 'country']} label="Country">
        <CountrySelect placeholder="Select country" />
      </Form.Item>

      <Form.Item name={['profile', 'sex']} label="Sex">
        <Select allowClear placeholder="Select" options={SEX_OPTIONS} />
      </Form.Item>

      <Form.Item name={['profile', 'civlId']} label="CIVL ID">
        <InputNumber
          min={1}
          controls={false}
          placeholder="e.g. 12345"
          style={{ width: '100%' }}
        />
      </Form.Item>

      <div className={styles.actions}>
        <Button onClick={onCancel} disabled={isSubmitting}>
          Cancel
        </Button>
        <Button
          type="primary"
          loading={isSubmitting}
          onClick={() => form.submit()}
        >
          Save
        </Button>
      </div>
    </Form>
  );
}

interface UserFormValues extends Record<string, unknown> {
  name: string;
  login: string;
  email: string;
  emailVerified: boolean;
  /** Selected permission flags; folded into a bitfield on submit. */
  permissions: number[];
  password: string;
  profile: {
    civlId: number | null;
    country: string | null;
    sex: UserSex | null;
  };
}

const PERMISSION_OPTIONS: { label: string; value: number }[] = [
  { label: 'Can log in', value: Permissions.CAN_AUTHORIZE },
  { label: 'Manage tracks', value: Permissions.MANAGE_TRACKS },
  { label: 'Manage users', value: Permissions.MANAGE_USERS },
  { label: 'Manage settings', value: Permissions.MANAGE_SETTINGS },
  { label: 'Manage gliders', value: Permissions.MANAGE_GLIDERS },
  { label: 'Manage sites', value: Permissions.MANAGE_SITES },
];

const bitsToFlags = (bits: number): number[] =>
  PERMISSION_OPTIONS.map((o) => o.value).filter(
    (flag) => (bits & flag) === flag,
  );

const flagsToBits = (flags: number[]): number =>
  flags.reduce((acc, flag) => acc | flag, 0);

const formInitial = (user: User | null): UserFormValues => ({
  name: user?.name ?? '',
  login: user?.login ?? '',
  email: user?.email ?? '',
  emailVerified: !!user?.emailVerifiedAt,
  permissions: bitsToFlags(user?.permissions ?? Permissions.CAN_AUTHORIZE),
  password: '',
  profile: {
    civlId: user?.profile?.civlId ?? null,
    country: user?.profile?.country ?? null,
    sex: user?.profile?.sex ?? null,
  },
});

const emptyToNull = (value: string | null | undefined): string | null => {
  const trimmed = value?.trim() ?? '';
  return trimmed === '' ? null : trimmed;
};

const normalize = (values: UserFormValues): UserInput => ({
  name: values.name.trim(),
  login: emptyToNull(values.login),
  email: emptyToNull(values.email),
  emailVerified: !!values.emailVerified,
  permissions: flagsToBits(values.permissions ?? []),
  // Empty field means "no change" (edit) / "no password yet" (create).
  password: values.password || null,
  profile: {
    civlId: values.profile?.civlId ?? null,
    country: values.profile?.country ?? null,
    sex: values.profile?.sex ?? null,
  },
});
