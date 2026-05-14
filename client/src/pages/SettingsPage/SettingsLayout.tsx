import {
  BarChartOutlined,
  ControlOutlined,
  LockOutlined,
  RiseOutlined,
  SettingOutlined,
  TeamOutlined,
  UserOutlined,
} from '@ant-design/icons';
import clsx from 'clsx';
import type { ReactNode } from 'react';
import { NavLink, Outlet } from 'react-router';
import { PageLayout } from '../../components/PageLayout';
import {
  type Permission,
  Permissions,
  hasPermission,
  useIdentity,
} from '../../core/identity';
import { routes } from '../../core/routes';
import styles from './SettingsLayout.module.scss';
import { SpIcon } from '../../components/icons';

interface NavItem {
  label: string;
  to: string;
  icon: ReactNode;
  /** Hide this entry unless `me` holds the bit. Omit for everyone. */
  permission?: Permission;
  /**
   * Stay active on child routes too (e.g. `/users/:id` lights up the
   * `Users` parent). Default off — most items have no sub-routes and
   * matching by prefix on those risks false positives.
   */
  matchPrefix?: boolean;
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

/**
 * Two-column shell shared by every `/settings/*` route. Each item can declare a
 * `permission`; items the viewer doesn't hold are dropped, and a group that
 * loses all of its items disappears with them.
 */
export function SettingsLayout() {
  const { me } = useIdentity();

  const groups: NavGroup[] = [
    {
      label: 'Account',
      items: [
        {
          label: 'Profile',
          to: routes.settings.profile(),
          icon: <UserOutlined />,
        },
        {
          label: 'Preferences',
          to: routes.settings.preferences(),
          icon: <ControlOutlined />,
        },
        {
          label: 'Authorization',
          to: routes.settings.authorization(),
          icon: <LockOutlined />,
        },
        {
          label: 'Stats',
          to: routes.settings.stats(),
          icon: <BarChartOutlined />,
        },
        {
          label: 'My flights',
          to: routes.settings.myFlights(),
          icon: <RiseOutlined />,
        },
        {
          label: 'My gliders',
          to: routes.settings.myGliders(),
          icon: <SpIcon />,
        },
      ],
    },
    {
      label: 'System',
      items: [
        {
          label: 'Settings',
          to: routes.settings.system(),
          icon: <SettingOutlined />,
          permission: Permissions.MANAGE_SETTINGS,
        },
        {
          label: 'Users',
          to: routes.settings.users(),
          icon: <TeamOutlined />,
          permission: Permissions.MANAGE_USERS,
          matchPrefix: true,
        },
        {
          label: 'Gliders',
          to: routes.settings.gliders(),
          icon: <SpIcon />,
          permission: Permissions.MANAGE_GLIDERS,
        },
      ],
    },
  ];

  const visibleGroups = groups
    .map((g) => ({
      ...g,
      items: g.items.filter(
        (item) =>
          !item.permission ||
          (me !== null && hasPermission(me, item.permission)),
      ),
    }))
    .filter((g) => g.items.length > 0);

  return (
    <PageLayout fit>
      <div className={styles.layout}>
        <nav className={styles.nav}>
          {visibleGroups.map((group) => (
            <div key={group.label} className={styles.group}>
              <span className={styles.groupLabel}>{group.label}</span>
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={!item.matchPrefix}
                  className={({ isActive }) =>
                    clsx(styles.item, isActive && styles.itemActive)
                  }
                >
                  <span className={styles.itemIcon}>{item.icon}</span>
                  <span className={styles.itemLabel}>{item.label}</span>
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
        <div className={styles.content}>
          <Outlet />
        </div>
      </div>
    </PageLayout>
  );
}
