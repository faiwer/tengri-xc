/**
 * Origins this run serves on, shared by `playwright.config.ts` and the specs.
 *
 * Read lazily rather than exported as constants: the config folds
 * `client/.env` into `process.env` on load, which happens after imports are
 * evaluated.
 */

export const getClientPort = (): number => envInt('E2E_CLIENT_PORT', 5174);

export const getServerPort = (): number => envInt('E2E_SERVER_PORT', 3001);

export const getClientOrigin = (): string =>
  `http://127.0.0.1:${getClientPort()}`;

/**
 * Public API base. `/api` because `main.rs` nests the whole router under it —
 * without the prefix both the readiness probe and every client call 404.
 */
export const getServerUrl = (): string =>
  process.env.E2E_SERVER_URL ?? `http://127.0.0.1:${getServerPort()}/api`;

/** Origin the browser is pointed at, and the one mailed links must resolve to. */
export const getBaseUrl = (): string =>
  process.env.E2E_BASE_URL ?? getClientOrigin();

function envInt(name: string, fallback: number): number {
  const raw = process.env[name]?.trim();
  const value = raw ? Number(raw) : fallback;
  if (!Number.isInteger(value)) {
    throw new Error(`${name} must be an integer, got ${raw}`);
  }
  return value;
}
