import clsx from 'clsx';
import { NavLink, Outlet } from 'react-router';
import { PageLayout } from '../../components/PageLayout';
import { isAdmin, useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import styles from './SettingsLayout.module.scss';

interface NavItem {
  label: string;
  to: string;
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

/**
 * Two-column shell shared by every `/settings/*` route. The System
 * group is gated on admin permission; everything else is the same
 * sidebar regardless of viewer.
 */
export function SettingsLayout() {
  const { me } = useIdentity();

  const groups: NavGroup[] = [
    {
      label: 'Account',
      items: [
        { label: 'Settings', to: routes.settings.profile() },
        { label: 'Authorization', to: routes.settings.authorization() },
        { label: 'Stats', to: routes.settings.stats() },
        { label: 'My flights', to: routes.settings.myFlights() },
      ],
    },
  ];

  if (me && isAdmin(me)) {
    groups.push({
      label: 'System',
      items: [
        { label: 'Settings', to: routes.settings.system() },
        { label: 'Users', to: routes.settings.users() },
      ],
    });
  }

  return (
    <PageLayout>
      <div className={styles.layout}>
        <nav className={styles.nav}>
          {groups.map((group) => (
            <div key={group.label} className={styles.group}>
              <span className={styles.groupLabel}>{group.label}</span>
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end
                  className={({ isActive }) =>
                    clsx(styles.item, isActive && styles.itemActive)
                  }
                >
                  {item.label}
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
