import styles from './ErrorPane.module.scss';
import clsx from 'clsx';
import { useEffect } from 'react';

interface ErrorPaneProps {
  title?: string;
  error: unknown;
  className?: string;
  valign?: 'top' | 'center';
}

export function ErrorPane({
  title = 'Oops. Something went wrong',
  valign = 'center',
  error,
  className,
}: ErrorPaneProps) {
  useEffect(() => {
    console.error(error);
  }, [error]);

  return (
    <div
      className={clsx(
        styles.container,
        className,
        valign === 'top' && styles.top,
      )}
    >
      <h3 className={styles.title}>{title}</h3>
      <img
        src="/images/errorMan.svg"
        alt="Error"
        className={styles.errorHead}
      />
    </div>
  );
}
