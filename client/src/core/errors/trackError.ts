/**
 * Context attached to a tracked error so the eventual sink (Sentry,
 * a backend log endpoint, …) can group / filter without us having to
 * stringify the error site at every call.
 *
 * Keep keys narrow and stable — they become tag dimensions later.
 */
export interface TrackErrorContext {
  /** Coarse area of the app: `'tracks-feed'`, `'flight-chart'`, `'map'`, … */
  feature: string;
  /** Where in that area the error came from: `'useAsyncEffect'`, hook name, function name. */
  origin: string;
  /** Free-form extras (track id, request URL, …). Avoid PII. */
  extra?: Record<string, unknown>;
}

/**
 * Single sink for "this should never have happened" errors.
 *
 * Today it just logs — there's no telemetry pipeline yet. When we wire
 * one up (Sentry, a `/log` endpoint, …) the change is one file: every
 * caller already speaks in terms of `TrackErrorContext`.
 */
export function trackError(err: unknown, ctx: TrackErrorContext): void {
  console.error(`[${ctx.feature}/${ctx.origin}]`, err, ctx.extra);
}
