import { Button } from 'antd';
import { FullscreenOutlined, FullscreenExitOutlined } from '@ant-design/icons';
import type { RefObject } from 'react';
import { useFullscreen } from '../../utils/useFullscreen';

interface MapFullscreenButtonProps {
  containerRef: RefObject<HTMLElement | null>;
}

export function MapFullscreenButton({
  containerRef,
}: MapFullscreenButtonProps) {
  const { isFullscreen, toggle } = useFullscreen(containerRef);
  return (
    <Button
      aria-label={isFullscreen ? 'Exit fullscreen' : 'Fullscreen'}
      icon={isFullscreen ? <FullscreenExitOutlined /> : <FullscreenOutlined />}
      onClick={toggle}
    />
  );
}
