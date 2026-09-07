import { Form, Input, InputNumber, Select } from 'antd';
import type { SmtpTls } from '../../../api/admin/site.io';

interface ConnectionFieldsProps {
  /**
   * Drives the password placeholder. The box itself is always empty — blank
   * means "leave unchanged" server-side — so this is the only hint the
   * operator gets that a password is already stored.
   */
  hasPassword: boolean;
}

/** Where and how to reach the relay. Renders into the parent `<Form>`. */
export function ConnectionFields({ hasPassword }: ConnectionFieldsProps) {
  return (
    <>
      <Form.Item
        name="smtpHost"
        label={<span>SMTP host</span>}
        tooltip="Hostname of the relay. Empty disables outgoing mail."
      >
        <Input
          autoComplete="off"
          placeholder="mail.example.com"
          maxLength={FIELD_MAX_LEN}
        />
      </Form.Item>
      <Form.Item
        name="smtpPort"
        label={<span>SMTP port</span>}
        tooltip="465 for implicit TLS, 587 for STARTTLS, 25 for plaintext."
      >
        <InputNumber min={PORT_MIN} max={PORT_MAX} placeholder="465" />
      </Form.Item>
      <Form.Item
        name="smtpTls"
        label={<span>Encryption</span>}
        tooltip="Implicit opens the connection inside TLS; STARTTLS upgrades a plaintext one."
      >
        <Select allowClear placeholder="Not set" options={TLS_OPTIONS} />
      </Form.Item>
      <Form.Item name="smtpUsername" label={<span>Username</span>}>
        <Input
          autoComplete="off"
          placeholder="noreply@example.com"
          maxLength={FIELD_MAX_LEN}
        />
      </Form.Item>
      <Form.Item
        name="smtpPassword"
        label={<span>Password</span>}
        tooltip="Write-only. Leaving it blank keeps whatever is already stored."
      >
        <Input.Password
          autoComplete="off"
          placeholder={
            hasPassword ? 'Leave blank to keep the current password' : 'Not set'
          }
          maxLength={FIELD_MAX_LEN}
        />
      </Form.Item>
      <Form.Item
        name="fromAddress"
        label={<span>From address</span>}
        tooltip="Envelope sender. Must be an address the relay is allowed to send as."
      >
        <Input
          type="email"
          autoComplete="off"
          placeholder="noreply@example.com"
          maxLength={FIELD_MAX_LEN}
        />
      </Form.Item>
    </>
  );
}

const TLS_OPTIONS: { value: SmtpTls; label: string }[] = [
  { value: 'implicit', label: 'Implicit TLS (SMTPS)' },
  { value: 'starttls', label: 'STARTTLS' },
  { value: 'none', label: 'None (plaintext)' },
];

const FIELD_MAX_LEN = 512;
const PORT_MIN = 1;
const PORT_MAX = 65535;
