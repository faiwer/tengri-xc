import clsx from 'clsx';
import type { ReactNode } from 'react';
import { Link } from 'react-router';
import type { TrackListItem } from '../../api/tracks.io';
import { routes } from '../../core/routes';
import styles from './TrackRow.module.scss';

/**
 * One cell of a {@link TrackRow}. The row wraps the {@link content} in a
 * `<Link>` to `/track/:id` so the entire visible cell becomes a hit
 * target with native middle-click / Cmd-click semantics — pilots will
 * open multiple flights in tabs, and intercepting clicks via `onRow`
 * would silently break that.
 *
 * Cells stay declarative and renderer-defined: the row primitive doesn't
 * know about column types (date / number / pilot name). Each consumer
 * defines its own column set; the home feed will have one shape, the
 * "find flight buddies" widget will have another.
 */
export interface TrackRowCell {
  /** Stable key within the row; reused as React's `key` for the cell. */
  key: string;
  /** Cell body. Free-form ReactNode so callers can drop in icons, badges, etc. */
  content: ReactNode;
  /** Right-align numerical / duration cells. */
  align?: 'left' | 'right';
  /**
   * Render the cell text in a muted/secondary color. Use for contextual
   * columns the eye should glide past (date, ids) so the primary data
   * (pilot name, duration) reads first.
   */
  muted?: boolean;
  /** Optional CSS classname merged onto the `<td>`. */
  className?: string;
}

interface TrackRowProps {
  item: TrackListItem;
  cells: TrackRowCell[];
}

export function TrackRow({ item, cells }: TrackRowProps) {
  const href = routes.track(item.track.id);

  return (
    <tr className={styles.row}>
      {cells.map((cell) => (
        <td
          key={cell.key}
          className={clsx(
            styles.cell,
            cell.muted && styles.cellMuted,
            cell.className,
          )}
          data-align={cell.align ?? 'left'}
        >
          <Link to={href} className={styles.link}>
            {cell.content}
          </Link>
        </td>
      ))}
    </tr>
  );
}
