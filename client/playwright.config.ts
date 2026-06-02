import { fileURLToPath } from 'node:url';
import { defineConfig, devices } from '@playwright/test';
import { loadEnv } from 'vite';

const CLIENT_ROOT = fileURLToPath(new URL('.', import.meta.url));
loadClientEnv();

const CLIENT_PORT = envInt('E2E_CLIENT_PORT', 5174);
const SERVER_PORT = envInt('E2E_SERVER_PORT', 3001);
const CLIENT_ORIGIN = `http://127.0.0.1:${CLIENT_PORT}`;
const SERVER_URL =
  process.env.E2E_SERVER_URL ?? `http://127.0.0.1:${SERVER_PORT}`;
const BASE_URL = process.env.E2E_BASE_URL ?? CLIENT_ORIGIN;
const E2E_LOCALE = 'en-DE-u-hc-h23';
const START_SERVERS = process.env.E2E_START_SERVERS !== '0';
const NEED_DATABASE_URL =
  START_SERVERS || process.env.E2E_SKIP_DB_RESET !== '1';
const DATABASE_URL = NEED_DATABASE_URL ? requiredEnv('E2E_DATABASE_URL') : '';

export default defineConfig({
  testDir: './e2e',
  globalSetup: './e2e/support/globalSetup.ts',
  fullyParallel: false,
  workers: 1,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: BASE_URL,
    locale: E2E_LOCALE,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: START_SERVERS
    ? [
        {
          // Start the Rust API server against the E2E database.
          command: [
            `DATABASE_URL=${shellQuote(DATABASE_URL)}`,
            `SERVER_ADDR=127.0.0.1:${SERVER_PORT}`,
            `CLIENT_ORIGINS=${shellQuote(CLIENT_ORIGIN)}`,
            'cargo run --manifest-path ../server/Cargo.toml --bin tengri-server',
          ].join(' '),
          // Playwright waits for this readiness URL before running tests.
          url: `${SERVER_URL}/health`,
          timeout: 120_000,
          reuseExistingServer: false,
          stdout: 'pipe',
          stderr: 'pipe',
        },
        {
          // Start the Vite client pointed at the E2E API server.
          command: [
            `VITE_SERVER_URL=${shellQuote(SERVER_URL)}`,
            `VITE_GOOGLE_MAPS_API_KEY=${shellQuote(
              process.env.VITE_GOOGLE_MAPS_API_KEY ?? '',
            )}`,
            `vite --host 127.0.0.1 --port ${CLIENT_PORT} --strictPort`,
          ].join(' '),
          // Playwright waits for this readiness URL before running tests.
          url: BASE_URL,
          timeout: 60_000,
          reuseExistingServer: false,
          stdout: 'pipe',
          stderr: 'pipe',
        },
      ]
    : undefined,
});

function envInt(name: string, fallback: number): number {
  const raw = process.env[name]?.trim();
  const value = raw ? Number(raw) : fallback;
  if (!Number.isInteger(value)) {
    throw new Error(`${name} must be an integer, got ${raw}`);
  }
  return value;
}

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required for E2E tests`);
  }
  return value;
}

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function loadClientEnv(): void {
  const env = loadEnv(process.env.MODE ?? 'development', CLIENT_ROOT, '');
  for (const [key, value] of Object.entries(env)) {
    process.env[key] ??= value;
  }
}
