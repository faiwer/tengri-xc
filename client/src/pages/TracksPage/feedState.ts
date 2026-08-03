import type { TrackListItem } from '../../api/tracks.io';

/**
 * Snapshot of the tracks feed stashed in the `/tracks` history entry so a Back
 * navigation from a flight page restores it without refetching. Written only
 * when leaving via a row click; read once on mount.
 */
export interface FeedSnapshot {
  /** Accumulated rows across every page fetched so far. */
  items: TrackListItem[];
  /** Cursor of the most recently fetched page; `null` for the first page. */
  cursor: string | null;
  /** Cursor for the next page, or `null` on the last page. */
  nextCursor: string | null;
  /** `window.scrollY` at the moment of leaving. */
  scrollTop: number;
  /** `Date.now()` at write; drives the freshness check. */
  savedAt: number;
}

/**
 * The snapshot when one is present and younger than {@link FEED_STATE_TTL_MS},
 * else `null`. No shape validation — we own every write, so the only questions
 * are "is one there" and "is it stale".
 */
export function readFeedSnapshot(state: unknown): FeedSnapshot | null {
  if (
    state == null ||
    typeof state !== 'object' ||
    !('savedAt' in state) ||
    typeof state.savedAt !== 'number' ||
    Date.now() - state.savedAt >= FEED_STATE_TTL_MS
  ) {
    return null;
  }

  return state as FeedSnapshot;
}

/**
 * Persist `snapshot` into the current history entry silently — raw
 * `replaceState` rather than `navigate({ replace })`, so it triggers no router
 * re-render. `usr` is react-router's slot for `location.state`; spreading the
 * existing state keeps its `key`/`idx` bookkeeping intact.
 */
export function writeFeedSnapshot(snapshot: FeedSnapshot | null): void {
  window.history.replaceState({ ...window.history.state, usr: snapshot }, '');
}

const FEED_STATE_TTL_MS = 5 * 60_000;
