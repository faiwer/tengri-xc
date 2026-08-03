import { useRef, useState } from 'react';
import { useAsyncEffect } from '../core/hooks';
import { nullthrows } from './nullthrows';

type Comparator = '<' | '<=' | '>' | '>=';

/** A width predicate, e.g. `['<', 1000]` reads as "viewport narrower than 1000px". */
type MediaRule = [Comparator, number];

/**
 * Resolve the current window width to the first matching key in `rules`. Rules
 * are evaluated in insertion order; the object is treated as a const (captured
 * once via a ref), and the hook only re-renders when the resolved key changes.
 *
 * @example
 * const layout = useMediaQuery({
 *   tiny: ['<', 500],
 *   medium: ['<', 1000],
 *   large: ['>=', 1000],
 * });
 * // layout: 'tiny' | 'medium' | 'large'
 */
export function useMediaQuery<K extends string>(
  rules: Record<K, MediaRule>,
): K {
  const rulesRef = useRef(rules);
  const [key, setKey] = useState<K>(() =>
    resolve(rulesRef.current, window.innerWidth),
  );

  useAsyncEffect(() => {
    const onResize = () => {
      setKey(resolve(rulesRef.current, window.innerWidth));
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  return key;
}

const matches = (width: number, [op, px]: MediaRule): boolean => {
  switch (op) {
    case '<':
      return width < px;
    case '<=':
      return width <= px;
    case '>':
      return width > px;
    case '>=':
      return width >= px;
  }
};

function resolve<K extends string>(
  rules: Record<K, MediaRule>,
  width: number,
): K {
  const entries = Object.entries(rules) as [K, MediaRule][];
  const found = entries.find(([, rule]) => matches(width, rule));
  // Fall back to the last key so the return type stays `K`; exhaustive
  // rulesets never reach this.
  return nullthrows(found ?? entries.at(-1))[0];
}
