import { Button, Form, InputNumber, Segmented, Select, Skeleton } from 'antd';
import { useMemo } from 'react';
import { Navigate } from 'react-router';
import { updateMe } from '../../api/users';
import type {
  MeProfile,
  UpdateProfileRequest,
  UserSex,
} from '../../api/users.io';
import { LoadError } from '../../components/LoadError';
import { useFormSubmit } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import { countryOptions } from '../../utils/formatCountry';
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

  return <ProfileForm initial={profileInitial(me.profile)} onSaved={setMe} />;
}

interface ProfileFormProps {
  initial: ProfileFormValues;
  onSaved: (me: NonNullable<ReturnType<typeof useIdentity>['me']>) => void;
}

interface ProfileFormValues extends Record<string, unknown> {
  civlId: number | null;
  country: string | null;
  sex: UserSex | null;
}

function ProfileForm({ initial, onSaved }: ProfileFormProps) {
  const [form] = Form.useForm<ProfileFormValues>();
  const countries = useMemo(countryOptions, []);

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values) => updateMe({ profile: normalizeProfile(values) }),
    onSuccess: onSaved,
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
          name="sex"
          label="Sex"
          rules={[{ required: true, message: 'Choose a value' }]}
        >
          <Segmented block options={SEX_OPTIONS} />
        </Form.Item>

        <Form.Item name="country" label="Country">
          <Select
            allowClear
            showSearch
            optionFilterProp="label"
            placeholder="Select country"
            options={countries.map((country) => ({
              value: country.code,
              label: country.label,
            }))}
          />
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

const SEX_OPTIONS: { label: string; value: UserSex }[] = [
  { label: 'Male', value: 'male' },
  { label: 'Female', value: 'female' },
  { label: 'Diverse', value: 'diverse' },
];

const profileInitial = (profile: MeProfile | null): ProfileFormValues => ({
  civlId: profile?.civlId ?? null,
  country: profile?.country ?? null,
  sex: profile?.sex ?? null,
});

const normalizeProfile = (values: ProfileFormValues): UpdateProfileRequest => ({
  civlId: values.civlId ?? null,
  country: values.country ?? null,
  sex: values.sex,
});
