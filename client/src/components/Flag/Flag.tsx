import {
  type FormattedCountry,
  formatCountry,
} from '../../utils/formatCountry';
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
  /**
   * Render a placeholder glyph when `code` is missing or unrecognised
   * instead of nothing. Use to keep neighbouring text aligned across
   * rows where some entries have a flag and some don't.
   *
   * - `'white'` — neutral white flag (🏳️). Visually matches the
   *   sibling country flags (same height, same shape).
   * - `'world'` — globe (🌐). Reads as "place / origin unknown" but
   *   stands out next to country flags.
   *
   * Default omitted: render `null`.
   */
  fallback?: 'white' | 'world';
}

const FALLBACKS: Record<
  NonNullable<FlagProps['fallback']>,
  FormattedCountry
> = {
  white: { flag: '🏳️', name: 'Unknown' },
  world: { flag: '🌐', name: 'Unknown' },
};

/**
 * Country flag emoji with a hover tooltip + accessible label of the
 * localized country name. Returns `null` when the code is missing or
 * unrecognised by the runtime unless [`fallback`](FlagProps.fallback)
 * picks a placeholder.
 */
export const Flag = ({ code, decorative = false, fallback }: FlagProps) => {
  const formatted =
    (code ? formatCountry(code) : null) ??
    (fallback ? FALLBACKS[fallback] : null);

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
