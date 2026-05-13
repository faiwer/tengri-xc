import { z } from 'zod';

import { SPORTS, type Sport } from '../../core/sport';

/**
 * Zod for {@link Sport}. The admin glider-dictionary endpoint narrows further
 * server-side (rejects `'other'`, which has no model list); this schema is the
 * broad one used for FE-wide things like persisted filter state.
 */
export const SportIo = z.enum(SPORTS);
export type { Sport };
