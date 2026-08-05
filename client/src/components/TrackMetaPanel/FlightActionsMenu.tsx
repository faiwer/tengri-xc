import { Button, ConfigProvider, Dropdown, theme } from 'antd';
import type { MenuProps } from 'antd';
import {
  DeleteOutlined,
  EditOutlined,
  EllipsisOutlined,
} from '@ant-design/icons';
import { useRemoveFlight } from './useRemoveFlight';

export function FlightActionsMenu({
  flightId,
  anchorClassName,
  onEdit,
}: {
  flightId: string;
  anchorClassName?: string;
  onEdit: () => void;
}) {
  const removeFlight = useRemoveFlight(flightId);

  const onClick: MenuProps['onClick'] = ({ key }) => {
    if (key === 'edit') {
      onEdit();
    } else if (key === 'remove') {
      removeFlight();
    }
  };

  return (
    <ConfigProvider theme={{ algorithm: theme.darkAlgorithm }}>
      <Dropdown menu={{ items: MENU_ITEMS, onClick }} trigger={['click']}>
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

const MENU_ITEMS: MenuProps['items'] = [
  { key: 'edit', label: 'Edit flight', icon: <EditOutlined /> },
  {
    key: 'remove',
    label: 'Remove flight',
    icon: <DeleteOutlined />,
    danger: true,
  },
];
