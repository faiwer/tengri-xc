import { Button, Skeleton } from 'antd';
import { useState } from 'react';
import { Link, useParams } from 'react-router';

import { getUser } from '../../api/admin/users';
import type { User } from '../../api/admin/users.io';
import type { UserSex, UserSource } from '../../api/users.io';
import { Flag } from '../../components/Flag';
import { LoadError } from '../../components/LoadError';
import {
  useAsync,
  useAsyncEffect,
  useErrorToast,
  useEventHandler,
} from '../../core/hooks';
import {
  usePreferences,
  type ResolvedPreferences,
} from '../../core/preferences';
import { routes } from '../../core/routes';
import { formatCountry } from '../../utils/formatCountry';
import { formatShortDate, formatShortTime } from '../../utils/formatDateTime';
import { PermissionBadges } from './PermissionBadges';
import { SettingsSection } from './SettingsSection';
import styles from './UserDetailSettings.module.scss';

export function UserDetailSettings() {
  const { id: rawId } = useParams<{ id: string }>();
  const id = parseUserId(rawId);
  const prefs = usePreferences();

  const [user, setUser] = useState<User | null>(null);
  const [fetchUser, , error] = useAsync(getUser);
  const [retryToken, setRetryToken] = useState(0);

  useAsyncEffect(
    async (signal) => {
      setUser(null);
      const next = await fetchUser(id, { signal });
      if (!signal.aborted) {
        setUser(next);
      }
    },
    [id, retryToken],
  );

  useErrorToast(error, { title: "Couldn't load user" });

  const retry = useEventHandler(() => setRetryToken((t) => t + 1));

  if (user === null && error !== null) {
    return (
      <LoadError
        title="Couldn't load user"
        error={error}
        onRetry={retry}
        extraActions={
          <Link to={routes.settings.users()}>
            <Button size="small">Back to users</Button>
          </Link>
        }
      />
    );
  }

  if (user === null) {
    return <Skeleton active paragraph={{ rows: 8 }} />;
  }

  return (
    <SettingsSection
      title={user.name}
      action={
        <Link to={routes.settings.users()} className={styles.back}>
          ← All users
        </Link>
      }
    >
      <h3 className={styles.subtitle}>Account</h3>
      <dl className={styles.list}>
        <Row label="ID">{user.id}</Row>
        <Row label="Name">{user.name}</Row>
        <Row label="Login">{user.login ?? <Muted>—</Muted>}</Row>
        <Row label="Email">{user.email ?? <Muted>—</Muted>}</Row>
        <Row label="Source">{formatSource(user.source)}</Row>
        <Row label="Permissions">
          <PermissionBadges bits={user.permissions} />
        </Row>
        <Row label="Created">{formatTimestamp(user.createdAt, prefs)}</Row>
        <Row label="Last login">
          {user.lastLoginAt === null ? (
            <Muted>never</Muted>
          ) : (
            formatTimestamp(user.lastLoginAt, prefs)
          )}
        </Row>
        <Row label="Email verified">
          {user.emailVerifiedAt === null ? (
            <Muted>no</Muted>
          ) : (
            formatTimestamp(user.emailVerifiedAt, prefs)
          )}
        </Row>
      </dl>

      <h3 className={styles.subtitle}>Profile</h3>
      {user.profile === null ? (
        <p className={styles.empty}>No profile data.</p>
      ) : (
        <dl className={styles.list}>
          <Row label="CIVL ID">{user.profile.civlId ?? <Muted>—</Muted>}</Row>
          <Row label="Country">{renderCountry(user.profile.country)}</Row>
          <Row label="Sex">
            {user.profile.sex === null ? (
              <Muted>—</Muted>
            ) : (
              formatSex(user.profile.sex)
            )}
          </Row>
        </dl>
      )}
    </SettingsSection>
  );
}

function parseUserId(raw: string | undefined): number {
  const id = Number.parseInt(raw ?? '', 10);
  if (!Number.isInteger(id)) {
    throw new Error(`Invalid user id: ${JSON.stringify(raw)}`);
  }
  return id;
}

const Row = ({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) => (
  <>
    <dt className={styles.term}>{label}</dt>
    <dd className={styles.def}>{children}</dd>
  </>
);

const Muted = ({ children }: { children: React.ReactNode }) => (
  <span className={styles.muted}>{children}</span>
);

const SOURCE_LABEL: Record<UserSource, string> = {
  internal: 'Internal',
  leo: 'Leonardo (imported)',
};

const SEX_LABEL: Record<UserSex, string> = {
  male: 'Male',
  female: 'Female',
  diverse: 'Diverse',
};

const formatSource = (source: UserSource): string =>
  SOURCE_LABEL[source] ?? source;

const formatSex = (sex: UserSex): string => SEX_LABEL[sex] ?? sex;

const formatTimestamp = (
  epochSeconds: number,
  prefs: ResolvedPreferences,
): string =>
  `${formatShortDate(epochSeconds, prefs)} ${formatShortTime(epochSeconds, prefs)}`;

const renderCountry = (code: string | null): React.ReactNode => {
  if (code === null) {
    return <Muted>—</Muted>;
  }

  const formatted = formatCountry(code);
  if (formatted === null) {
    // Code is malformed or the runtime doesn't know it (rare ISO
    // assignments). Show the raw code so we don't hide data.
    return code;
  }

  return (
    <>
      <Flag code={code} decorative />
      <span className={styles.countryName}>{formatted.name}</span>
    </>
  );
};
