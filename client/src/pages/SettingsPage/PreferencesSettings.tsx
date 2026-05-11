import { Button, Form, Segmented, Skeleton } from 'antd';
import { useMemo } from 'react';
import { Navigate } from 'react-router';
import { updateMe } from '../../api/users';
import type { UpdatePreferencesRequest } from '../../api/users.io';
import { useFormSubmit } from '../../core/hooks';
import { useIdentity } from '../../core/identity';
import { resolvePreferences } from '../../core/preferences';
import { routes } from '../../core/routes';
import { shallowEqual } from '../../utils/shallowEqual';
import { SettingsSection } from './SettingsSection';

/**
 * Owner-self settings page at `/settings/preferences`. Exposes the user's
 * display preferences as an AntD `Form` of {@link Segmented} controls.
 */
export function PreferencesSettings() {
  const { me, isLoading, setMe } = useIdentity();

  if (isLoading) {
    return <Skeleton active paragraph={{ rows: 8 }} />;
  } else if (!me) {
    return <Navigate replace to={routes.login()} />;
  }

  return <PreferencesForm initial={me.preferences} onSaved={setMe} />;
}

interface PreferencesFormProps {
  initial: UpdatePreferencesRequest;
  onSaved: (me: NonNullable<ReturnType<typeof useIdentity>['me']>) => void;
}

function PreferencesForm({ initial, onSaved }: PreferencesFormProps) {
  const [form] = Form.useForm<UpdatePreferencesRequest>();
  const systemHints = useMemo(() => resolvePreferences(null), []);

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values) => updateMe({ preferences: values }),
    onSuccess: onSaved,
    fieldPrefix: 'preferences',
    successTitle: 'Preferences saved',
    errorTitle: "Couldn't save preferences",
  });

  const values = Form.useWatch([], form) as
    | UpdatePreferencesRequest
    | undefined;
  const isDirty = useMemo(
    () => !!values && !shallowEqual(values, initial),
    [values, initial],
  );

  return (
    <SettingsSection
      title="Preferences"
      subtitle="Pick how dates, times, and units render across the app."
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
        labelCol={{ flex: '11rem' }}
        labelAlign="left"
        wrapperCol={{ flex: '1 1 auto' }}
        initialValues={initial}
        onFinish={onFinish}
      >
        <Form.Item
          name="units"
          label={<span>Altitude & distance</span>}
          tooltip="One choice for both — m + km vs ft + mi."
        >
          <Segmented
            block
            options={[
              {
                label: `System (${UNITS_LABEL[systemHints.units]})`,
                value: 'system',
              },
              { label: 'Metric', value: 'metric' },
              { label: 'Imperial', value: 'imperial' },
            ]}
          />
        </Form.Item>

        <Form.Item name="varioUnit" label="Vario">
          <Segmented
            block
            options={[
              {
                label: `System (${VARIO_LABEL[systemHints.varioUnit]})`,
                value: 'system',
              },
              { label: 'm/s', value: 'mps' },
              { label: 'ft/min', value: 'fpm' },
            ]}
          />
        </Form.Item>

        <Form.Item name="speedUnit" label="Ground speed">
          <Segmented
            block
            options={[
              {
                label: `System (${SPEED_LABEL[systemHints.speedUnit]})`,
                value: 'system',
              },
              { label: 'km/h', value: 'kmh' },
              { label: 'mph', value: 'mph' },
            ]}
          />
        </Form.Item>

        <Form.Item name="timeFormat" label="Time format">
          <Segmented
            block
            options={[
              {
                label: `System (${TIME_FORMAT_LABEL[systemHints.timeFormat]})`,
                value: 'system',
              },
              { label: '12-hour', value: 'h12' },
              { label: '24-hour', value: 'h24' },
            ]}
          />
        </Form.Item>

        <Form.Item name="dateFormat" label="Date format">
          <Segmented
            block
            options={[
              {
                label: `System (${DATE_FORMAT_LABEL[systemHints.dateFormat]})`,
                value: 'system',
              },
              { label: 'Day/Month', value: 'dmy' },
              { label: 'Month/Day', value: 'mdy' },
            ]}
          />
        </Form.Item>

        <Form.Item name="weekStart" label="Week starts on">
          <Segmented
            block
            options={[
              {
                label: `System (${WEEK_START_LABEL[systemHints.weekStart]})`,
                value: 'system',
              },
              { label: 'Monday', value: 'mon' },
              { label: 'Sunday', value: 'sun' },
            ]}
          />
        </Form.Item>
      </Form>
    </SettingsSection>
  );
}

const UNITS_LABEL = { metric: 'metric', imperial: 'imperial' } as const;
const VARIO_LABEL = { mps: 'm/s', fpm: 'ft/min' } as const;
const SPEED_LABEL = { kmh: 'km/h', mph: 'mph' } as const;
const TIME_FORMAT_LABEL = { h12: '12-hour', h24: '24-hour' } as const;
const DATE_FORMAT_LABEL = { dmy: 'Day/Month', mdy: 'Month/Day' } as const;
const WEEK_START_LABEL = { mon: 'Monday', sun: 'Sunday' } as const;
