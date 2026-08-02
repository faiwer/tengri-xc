import { Button, Form, Input, InputNumber } from 'antd';
import { useMemo } from 'react';

import { createSite, updateSite } from '../../../../api/admin/sites';
import type { SiteInput, SiteListItem } from '../../../../api/admin/sites.io';
import { CountrySelect } from '../../../../components/CountrySelect';
import { useFormSubmit } from '../../../../core/hooks';
import { nullthrows } from '../../../../utils/nullthrows';
import styles from './SiteForm.module.scss';

interface SiteFormProps {
  /** The site being edited, or `null` to create a new one. */
  site: SiteListItem | null;
  onSaved: (site: SiteListItem) => void;
  onCancel: () => void;
}

export function SiteForm({ site, onSaved, onCancel }: SiteFormProps) {
  const [form] = Form.useForm<SiteFormValues>();
  const initial = useMemo(() => formInitial(site), [site]);

  const { onFinish, isSubmitting } = useFormSubmit({
    form,
    submit: (values) => {
      const input = normalize(values);
      return site ? updateSite(site.id, input) : createSite(input);
    },
    onSuccess: onSaved,
    successTitle: site ? 'Site saved' : 'Site created',
    errorTitle: "Couldn't save site",
  });

  return (
    <Form
      form={form}
      layout="horizontal"
      labelCol={{ flex: '5rem' }}
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
        <Input placeholder="Site name" />
      </Form.Item>

      <Form.Item
        name="lat"
        label="Lat"
        rules={[
          { required: true, message: 'Enter a latitude' },
          { type: 'number', min: -90, max: 90, message: 'Between -90 and 90' },
        ]}
      >
        <InputNumber
          min={-90}
          max={90}
          controls={false}
          placeholder="47.72843"
          style={{ width: '100%' }}
        />
      </Form.Item>

      <Form.Item
        name="lng"
        label="Lng"
        rules={[
          { required: true, message: 'Enter a longitude' },
          {
            type: 'number',
            min: -180,
            max: 180,
            message: 'Between -180 and 180',
          },
        ]}
      >
        <InputNumber
          min={-180}
          max={180}
          controls={false}
          placeholder="12.63858"
          style={{ width: '100%' }}
        />
      </Form.Item>

      <Form.Item name="country" label="Country">
        <CountrySelect placeholder="Select country" />
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

interface SiteFormValues extends Record<string, unknown> {
  name: string;
  lat: number | null;
  lng: number | null;
  country: string | null;
}

const formInitial = (site: SiteListItem | null): SiteFormValues => ({
  name: site?.name ?? '',
  lat: site?.lat ?? null,
  lng: site?.lng ?? null,
  country: site?.country ?? null,
});

// `lat`/`lng` are `required` in the form, so `onFinish` only runs once they're
// present; `nullthrows` turns the residual nullable type into a hard value.
const normalize = (values: SiteFormValues): SiteInput => ({
  name: values.name.trim(),
  lat: nullthrows(values.lat),
  lng: nullthrows(values.lng),
  country: values.country ?? null,
});
