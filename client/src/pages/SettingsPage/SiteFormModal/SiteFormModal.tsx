import { Modal } from 'antd';

import type { SiteListItem } from '../../../api/admin/sites.io';
import { SiteForm } from './SiteForm';

interface SiteFormModalProps {
  open: boolean;
  /** The site being edited, or `null` to create a new one. */
  site: SiteListItem | null;
  onSaved: (site: SiteListItem) => void;
  onClose: () => void;
}

export function SiteFormModal({
  open,
  site,
  onSaved,
  onClose,
}: SiteFormModalProps) {
  return (
    <Modal
      title={site ? 'Edit site' : 'Add site'}
      open={open}
      footer={null}
      // Remount the form on each open so it re-reads `initialValues` for the
      // current target instead of keeping the previous edit's values.
      destroyOnHidden
      onCancel={onClose}
    >
      <SiteForm site={site} onSaved={onSaved} onCancel={onClose} />
    </Modal>
  );
}
