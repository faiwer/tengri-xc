import { Button, ConfigProvider, Dropdown, theme } from 'antd';
import type { MenuProps } from 'antd';
import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  EllipsisOutlined,
  UserSwitchOutlined,
} from '@ant-design/icons';
import { SERVER_URL } from '../../api/core';
import { transferFlight } from '../../api/me/flights';
import { useRemoveFlight } from './useRemoveFlight';

export function FlightActionsMenu({
  flightId,
  anchorClassName,
  onEdit,
  canTransfer,
}: {
  flightId: string;
  anchorClassName?: string;
  onEdit: () => void;
  canTransfer: boolean;
}) {
  const removeFlight = useRemoveFlight(flightId);

  const onClick: MenuProps['onClick'] = ({ key }) => {
    if (key === 'edit') {
      onEdit();
    } else if (key === 'transfer') {
      void promptTransferFlight(flightId);
    } else if (key === 'remove') {
      removeFlight();
    }
  };

  const items: MenuProps['items'] = [
    { key: 'edit', label: 'Edit flight', icon: <EditOutlined /> },
    {
      key: 'download',
      icon: <DownloadOutlined />,
      label: (
        <a
          href={`${SERVER_URL}/me/flights/${flightId}/source`}
          target="_blank"
          rel="noopener"
        >
          Download original track
        </a>
      ),
    },
    canTransfer && {
      key: 'transfer',
      label: 'Transfer flight',
      icon: <UserSwitchOutlined />,
    },
    {
      key: 'remove',
      label: 'Remove flight',
      icon: <DeleteOutlined />,
      danger: true,
    },
  ].filter((item) => !!item);

  return (
    <ConfigProvider theme={{ algorithm: theme.darkAlgorithm }}>
      <Dropdown menu={{ items, onClick }} trigger={['click']}>
        <Button
          type="text"
          className={anchorClassName}
          icon={<EllipsisOutlined />}
          aria-label="Flight actions"
        />
      </Dropdown>
    </ConfigProvider>
  );
}

/**
 * Ask for a target user id and hand the flight over, then hard-reload so the
 * page reflects the new owner. Cancelling the prompt is a no-op; a non-numeric
 * entry throws.
 */
async function promptTransferFlight(flightId: string): Promise<void> {
  const input = window.prompt('Transfer flight to user id:');
  if (/^\d+$/.test(input ?? '')) {
    await transferFlight(flightId, Number(input));
    window.location.reload();
  }
}
