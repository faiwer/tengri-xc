import {
  App,
  Button,
  Form,
  Input,
  InputNumber,
  Segmented,
  Skeleton,
} from 'antd';
import { useMemo } from 'react';
import { Navigate } from 'react-router';
import { updateMe } from '../../api/users';
import type { Me, UpdateProfileRequest, UserSex } from '../../api/users.io';
import { CountrySelect } from '../../components/CountrySelect';
import { LoadError } from '../../components/LoadError';
import { useFormSubmit } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import { SEX_OPTIONS } from '../../core/sex';
import { shallowEqual } from '../../utils/shallowEqual';
import { SettingsSection } from './SettingsSection';

export function ProfileSettings() {
  const { me, isLoading, error, retry, setMe } = useIdentity();

  if (isLoading) {
    return <Skeleton active paragraph={{ rows: 5 }} />;
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

  return <ProfileForm initial={profileInitial(me)} onSaved={setMe} />;
}

interface ProfileFormProps {
  initial: ProfileFormValues;
  onSaved: (me: NonNullable<ReturnType<typeof useIdentity>['me']>) => void;
}

interface ProfileFormValues extends Record<string, unknown> {
  name: string;
  email: string;
  civlId: number | null;
  country: string | null;
  sex: UserSex | null;
}

function ProfileForm({ initial, onSaved }: ProfileFormProps) {
  const [form] = Form.useForm<ProfileFormValues>();
  const { notification } = App.useApp();

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values) => updateMe({ profile: normalizeProfile(values) }),
    onSuccess: ({ emailVerificationReset, ...me }) => {
      onSaved(me);
      if (emailVerificationReset) {
        notification.warning({
          title: 'Verify your new email',
          description:
            'Your email address changed, so it needs to be verified again.',
          placement: 'bottomRight',
        });
      }
    },
    fieldPrefix: 'profile',
    successTitle: 'Profile saved',
    errorTitle: "Couldn't save profile",
  });

  const values = Form.useWatch([], form) as ProfileFormValues | undefined;
  const isDirty = useMemo(
    () => !!values && !shallowEqual(values, initial),
    [values, initial],
  );

  return (
    <SettingsSection
      title="Profile"
      subtitle="These fields identify you in rankings, lists, and public pilot views."
      action={
        isDirty && (
          <Button
            type="primary"
            loading={isSubmitting}
            onClick={() => form.submit()}
          >
            Save
          </Button>
        )
      }
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ flex: '7rem' }}
        labelAlign="left"
        wrapperCol={{ flex: '1 1 auto' }}
        initialValues={initial}
        onFinish={onFinish}
      >
        <Form.Item
          name="name"
          label="Name"
          rules={[{ required: true, message: 'Cannot be empty' }]}
        >
          <Input placeholder="Display name" />
        </Form.Item>

        <Form.Item name="email" label="Email">
          <Input type="email" placeholder="you@example.com" />
        </Form.Item>

        <Form.Item
          name="sex"
          label="Sex"
          rules={[{ required: true, message: 'Choose a value' }]}
        >
          <Segmented block options={SEX_OPTIONS} />
        </Form.Item>

        <Form.Item name="country" label="Country">
          <CountrySelect placeholder="Select country" />
        </Form.Item>

        <Form.Item name="civlId" label="CIVL ID">
          <InputNumber
            min={1}
            precision={0}
            controls={false}
            placeholder="CIVL pilot ID"
            style={{ width: '100%' }}
          />
        </Form.Item>
      </Form>
    </SettingsSection>
  );
}

const profileInitial = (me: Me): ProfileFormValues => ({
  name: me.name,
  // Coalesce to '' so an empty input matches `initial` and dirty-detection
  // doesn't false-fire on load for a user with no email.
  email: me.email ?? '',
  civlId: me.profile?.civlId ?? null,
  country: me.profile?.country ?? null,
  sex: me.profile?.sex ?? null,
});

const normalizeProfile = (values: ProfileFormValues): UpdateProfileRequest => ({
  name: values.name,
  email: values.email,
  civlId: values.civlId ?? null,
  country: values.country ?? null,
  sex: values.sex,
});
