import { useRef } from 'react';
import { useEventHandler } from '../../core/hooks';

/**
 * Wires an IntersectionObserver to whichever element the returned callback ref
 * is attached to. Fires `onReached` a viewport ahead of the sentinel hitting
 * the bottom of the scrollport.
 */
export function useScrollSentinel(onReached: () => void) {
  const handleReached = useEventHandler(onReached);
  const observerRef = useRef<IntersectionObserver | null>(null);

  return useEventHandler((node: HTMLElement | null) => {
    observerRef.current?.disconnect();
    observerRef.current = null;
    if (!node) {
      return;
    }

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
    observer.observe(node);
    observerRef.current = observer;
  });
}

const ROOT_MARGIN = '600px 0px';
