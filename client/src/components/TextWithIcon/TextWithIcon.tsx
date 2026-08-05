import { Tooltip } from 'antd';
import type { ReactNode } from 'react';
import { getCountryName } from '../../utils/formatCountry';
import { Flag } from '../Flag';
import styles from './TextWithIcon.module.scss';

export interface TextWithIconProps {
  /** Main content, shown after the optional leading icon/flag. */
  text: ReactNode;
  /** Leading icon. Ignored when {@link TextWithIconProps.flag} is set. */
  icon?: ReactNode;
  /**
   * ISO 3166-1 alpha-2 country code rendered as a leading flag with the
   * localized country-name tooltip. Takes precedence over
   * {@link TextWithIconProps.icon}; renders no flag when missing or
   * unrecognized.
   */
  flag?: string | null;
  /** Hover tooltip for {@link TextWithIconProps.text}. */
  tooltip?: ReactNode;
  /** Layout direction. */
  layout?: 'normal' | 'reverse';
}

/**
 * Inline "leading icon + text" pair. Stays inline (not inline-flex) so the
 * text keeps ellipsizing inside overflow-clipped containers like table cells.
 */
export const TextWithIcon = ({
  text,
  icon,
  flag,
  tooltip,
  layout = 'normal',
}: TextWithIconProps) => {
  const leading = flag ? (
    <Tooltip title={getCountryName(flag)}>
      <span>
        <Flag code={flag} />
      </span>
    </Tooltip>
  ) : (
    icon
  );

  const body =
    tooltip != null ? (
      <Tooltip title={tooltip}>
        <span>{text}</span>
      </Tooltip>
    ) : (
      text
    );

  const iconElement = leading != null && (
    <span className={styles.icon} data-layout={layout}>
      {leading}
    </span>
  );

  return layout === 'normal' ? (
    <>
      {iconElement}
      {body}
    </>
  ) : (
    <>
      {body}
      {iconElement}
    </>
  );
};
