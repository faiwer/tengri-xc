import { type RefObject, useState } from 'react';
import { nullthrows } from './nullthrows';
import { useEventHandler } from '../core/hooks/useEventHandler';
import { useAsyncEffect } from '../core/hooks';

export function useFullscreen(containerRef: RefObject<HTMLElement | null>) {
  const [isFullscreen, setFullscreen] = useState(false);

  const toggle = useEventHandler(() => {
    if (document.fullscreenElement) {
      document.exitFullscreen();
    } else {
      nullthrows(containerRef.current).requestFullscreen();
    }
  });

  useAsyncEffect(() => {
    const onChange = () => {
      setFullscreen(document.fullscreenElement === containerRef.current);
    };
    document.addEventListener('fullscreenchange', onChange);
    return () => document.removeEventListener('fullscreenchange', onChange);
  });

  return { isFullscreen, toggle };
}
