import { Form, Input } from 'antd';

/**
 * Wrappers applied to every outgoing message. The placeholder rules mirror the
 * server's so a typo surfaces before the round trip. Renders into the parent
 * `<Form>`.
 */
export function TemplateFields() {
  return (
    <>
      <Form.Item
        name="titleTemplate"
        label={<span>Subject template</span>}
        tooltip="Wraps every outgoing subject line. %title% is where the message's own subject lands."
        rules={[
          { required: true, message: 'Required' },
          { pattern: /%title%/, message: 'Must contain %title%' },
        ]}
      >
        <Input.TextArea autoSize={{ minRows: 1, maxRows: 4 }} />
      </Form.Item>
      <Form.Item
        name="bodyTemplate"
        label={<span>Body template</span>}
        tooltip="Wraps every outgoing body. %body% is where the message's own text lands."
        rules={[
          { required: true, message: 'Required' },
          { pattern: /%body%/, message: 'Must contain %body%' },
        ]}
      >
        <Input.TextArea autoSize={{ minRows: 6, maxRows: 24 }} />
      </Form.Item>
    </>
  );
}
