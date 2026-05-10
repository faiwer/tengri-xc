import { formatCountry } from '../../utils/formatCountry';
import styles from './Flag.module.scss';

export interface FlagProps {
  /** ISO 3166-1 alpha-2 country code, e.g. "DE". */
  code: string | null | undefined;
  /**
   * When `true`, hide from the accessibility tree. Use when the
   * country name is rendered as text right next to the flag — without
   * this, screen readers say "Germany Germany". Default `false`: the
   * flag is the only signal and gets an `aria-label` of the name.
   */
  decorative?: boolean;
}

/**
 * Country flag emoji with a hover tooltip + accessible label of the
 * localized country name. Returns `null` when the code is missing or
 * unrecognised by the runtime, so consumers can decide whether to
 * substitute a placeholder.
 */
export const Flag = ({ code, decorative = false }: FlagProps) => {
  if (!code) {
    return null;
  }

  const formatted = formatCountry(code);
  if (formatted === null) {
    return null;
  }

  return (
    <span
      className={styles.flag}
      title={formatted.name}
      aria-label={decorative ? undefined : formatted.name}
      aria-hidden={decorative ? true : undefined}
    >
      {formatted.flag}
    </span>
  );
};
