import type { ReactNode } from 'react';
import styles from './SettingsSection.module.scss';

interface SettingsSectionProps {
  title: ReactNode;
  subtitle?: ReactNode;
  /**
   * Right-aligned slot in the header — Save buttons (only when dirty),
   * search inputs, "Add user" actions. Pages decide what belongs here;
   * the section just makes the spot consistent.
   */
  action?: ReactNode;
  children?: ReactNode;
}

/**
 * Page chrome shared by every `/settings/*` route: title + optional
 * subtitle on the left, optional action on the right, the page's
 * actual content (form, table, list) below.
 *
 * The header reserves a fixed minimum height so toggling the action
 * (a Save button that only shows when the form is dirty) doesn't
 * shift the layout.
 */
export function SettingsSection({
  title,
  subtitle,
  action,
  children,
}: SettingsSectionProps) {
  return (
    <section className={styles.section}>
      <header className={styles.header}>
        <div className={styles.headerText}>
          <h2 className={styles.title}>{title}</h2>
          {subtitle && <p className={styles.subtitle}>{subtitle}</p>}
        </div>
        {action && <div className={styles.action}>{action}</div>}
      </header>
      <div className={styles.body}>{children}</div>
    </section>
  );
}
