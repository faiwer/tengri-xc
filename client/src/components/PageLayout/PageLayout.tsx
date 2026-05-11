import {
  GithubOutlined,
  LogoutOutlined,
  SettingOutlined,
  UserOutlined,
} from '@ant-design/icons';
import { Button } from 'antd';
import clsx from 'clsx';
import { useEffect, type ReactNode } from 'react';
import { Link, useNavigate } from 'react-router';
import { logout } from '../../api/users';
import { useAsync, useErrorToast } from '../../core/hooks';
import { isAdmin, useIdentity } from '../../core/identity';
import { routes } from '../../core/routes';
import { useSite } from '../../core/site';
import styles from './PageLayout.module.scss';

interface PageLayoutProps {
  children: ReactNode;
  /**
   * Default: content card stretches to fill the viewport (so pages with
   * map+chart can size against a definite parent). When `true`, the card sizes
   * to its content — used by the settings shell so an empty stub page doesn't
   * leave a tall white expanse below it.
   */
  fit?: boolean;
}

/**
 * App-wide chrome: branded header on top (with sign-out for logged-in
 * users), page body below. Every route renders one. Width and gutters
 * are owned here so individual pages don't redefine the same shell.
 */
export function PageLayout({ children, fit = false }: PageLayoutProps) {
  const { me, setMe } = useIdentity();
  const { site } = useSite();
  const navigate = useNavigate();

  // Reflect the configured site name into the browser tab. Per-page titles
  // aren't a concern yet — every route shows the same name.
  useEffect(() => {
    document.title = site.siteName;
  }, [site.siteName]);

  const [signOut, isSigningOut, signOutError] = useAsync(async () => {
    await logout();
    setMe(null);
    navigate(routes.login());
  });

  useErrorToast(signOutError, { title: "Couldn't sign out" });

  return (
    <main className={styles.page}>
      <header className={styles.header}>
        <Link to={routes.home()} className={styles.titleLink}>
          <span className={styles.logo} aria-hidden="true" />
          <h1 className={styles.title}>{site.siteName}</h1>
        </Link>
        {me && (
          <span className={styles.actions}>
            <Link to={routes.settings.profile()}>
              <Button
                icon={isAdmin(me) ? <SettingOutlined /> : <UserOutlined />}
                aria-label="Account settings"
              />
            </Link>
            <Button
              icon={<LogoutOutlined />}
              loading={isSigningOut}
              onClick={signOut}
              aria-label="Sign out"
            />
          </span>
        )}
      </header>
      <div className={clsx(styles.content, fit && styles.contentFit)}>
        {children}
      </div>
      <footer className={styles.footer}>
        {site.hasTos && <Link to={routes.terms()}>Terms</Link>}
        {site.hasPrivacy && <Link to={routes.privacy()}>Privacy</Link>}
        <a
          href="https://github.com/faiwer/tengri-xc"
          target="_blank"
          rel="noopener noreferrer"
          className={styles.footerExternal}
        >
          <GithubOutlined /> GitHub
        </a>
      </footer>
    </main>
  );
}
