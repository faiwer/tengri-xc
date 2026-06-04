import { Modal } from 'antd';

interface UploadFlightModalProps {
  open: boolean;
  onClose: () => void;
}

export function UploadFlightModal({ open, onClose }: UploadFlightModalProps) {
  return (
    <Modal title="Upload flight" open={open} onCancel={onClose} onOk={onClose}>
      Hello
    </Modal>
  );
}
