import { useEffect, useRef } from 'react';
import { useEventHandler } from '../../core/hooks';
import { nullthrows } from '../../utils/nullthrows';

/**
 * Wires an IntersectionObserver to a sentinel element so we can fire
 * `onReached` a viewport ahead of the user actually hitting the bottom.
 * Returns the ref to attach to the sentinel `<div>`.
 */
export function useScrollSentinel(onReached: () => void) {
  const sentinelRef = useRef<HTMLDivElement | null>(null);
  const handleReached = useEventHandler(onReached);

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            handleReached();
          }
        }
      },
      { rootMargin: ROOT_MARGIN },
    );
    observer.observe(
      nullthrows(sentinelRef.current, 'sentinel ref unset on mount'),
    );

    return () => observer.disconnect();
  }, []);

  return sentinelRef;
}

const ROOT_MARGIN = '600px 0px';
