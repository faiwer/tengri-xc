import { useEffect, useRef } from 'react';
import { useEventHandler } from '../../core/hooks';

/**
 * Wires an IntersectionObserver to whichever element the returned
 * callback is attached to. Fires `onReached` a viewport ahead of the
 * sentinel hitting the bottom of the scrollport.
 */
export function useScrollSentinel(onReached: () => void) {
  const handleReached = useEventHandler(onReached);
  const observerRef = useRef<IntersectionObserver | null>(null);

  useEffect(() => {
    observerRef.current = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            handleReached();
          }
        }
      },
      { rootMargin: ROOT_MARGIN },
    );
    return () => {
      observerRef.current?.disconnect();
      observerRef.current = null;
    };
  }, []);

  return useEventHandler(function onRef(node: HTMLElement | null) {
    const observer = observerRef.current;
    if (node) {
      observer?.observe(node);
    } else {
      observer?.disconnect();
    }
  });
}

const ROOT_MARGIN = '600px 0px';
