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

  return (
    <>
      {leading != null && <span className={styles.icon}>{leading}</span>}
      {body}
    </>
  );
};
