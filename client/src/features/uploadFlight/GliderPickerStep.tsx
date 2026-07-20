import { useState } from 'react';
import { listRecentGliders } from '../../api/me/recentGliders';
import type { RecentGlider } from '../../api/me/recentGliders.io';
import { GliderKindIcon } from '../../components/icons/GliderKindIcon';
import { LoadingIcon } from '../../components/icons/LoadingIcon';
import { useAsyncEffect } from '../../core/hooks';
import { usePreferences } from '../../core/preferences';
import { formatShortDate } from '../../utils/formatDateTime';
import styles from './GliderPickerStep.module.scss';
import { trackError } from '../../core/errors/trackError';

export function GliderPickerStep({
  onSelect,
}: {
  onSelect: (glider: RecentGlider | null) => void;
}) {
  const prefs = usePreferences();
  const [gliders, setGliders] = useState<RecentGlider[] | null>(null);

  useAsyncEffect(async (signal) => {
    try {
      const list = await listRecentGliders({ signal });
      if (signal.aborted) {
        return;
      }

      // Nothing to copy from — skip straight to the details form.
      if (list.length === 0) {
        onSelect(null);
        return;
      }

      setGliders(list);
    } catch (err) {
      trackError(err, { feature: 'uploadFlight', origin: 'gliderPickerStep' });
      onSelect(null); // This form may be safely skipped.
    }
  }, []);

  if (!gliders) {
    return (
      <div className={styles.loading}>
        <LoadingIcon />
      </div>
    );
  }

  return (
    <ul className={styles.list}>
      {gliders.map((glider) => (
        <li key={`${glider.kind}-${glider.brandId}-${glider.modelId}`}>
          <button
            type="button"
            className={styles.item}
            onClick={() => onSelect(glider)}
          >
            <GliderKindIcon kind={glider.kind} className={styles.icon} />
            <span className={styles.date}>
              {formatShortDate(glider.takeoffAt, prefs)}
            </span>
            <span className={styles.brand}>{glider.brandName}</span>
            <span className={styles.model}>{glider.modelName}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
