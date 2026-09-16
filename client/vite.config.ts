/// <reference types="vitest" />
import { fileURLToPath } from 'node:url';

import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import { ssrHtml } from './vite/ssrHtml';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, CLIENT_ROOT, '');
  // `VITE_SERVER_URL` holds the API base, which may or may not carry the
  // `/api` prefix (dev and E2E disagree) — `ssrHtml` wants the bare origin.
  // Unset, or path-only as in prod builds, means there's no server to forward
  // to and Vite serves `index.html` itself.
  const serverOrigin = parseOrigin(env.VITE_SERVER_URL);

  return {
    // Root-served SPA in prod: an absolute base lets client-side deep links
    // (`/flights/123` reloaded from scratch) resolve `/assets/*` correctly.
    // Relative `./` stays for dev/preview.
    base: mode === 'production' ? '/' : './',
    plugins: [react(), serverOrigin ? ssrHtml(serverOrigin) : null],
    resolve: {
      alias: {
        // Consumed as source (git submodule); Vite bundles the TS directly.
        'tengri-maplibre': fileURLToPath(
          new URL('./packages/tengri-maplibre/src/index.ts', import.meta.url),
        ),
      },
    },
    css: {
      modules: {
        localsConvention: 'camelCase',
      },
    },
    test: {
      include: [
        'src/**/*.test.ts',
        'src/**/*.test.tsx',
        'packages/tengri-maplibre/src/**/*.test.ts',
      ],
      environment: 'node',
      setupFiles: ['src/test/setup.ts'],
      server: {
        deps: {
          // bincode-ts ships ESM with extensionless internal imports
          // (`import './utils'`), which Vite resolves but plain Node ESM does
          // not. Inlining routes the package through Vite's bundler so its
          // imports are rewritten before Node sees them.
          inline: ['bincode-ts'],
        },
      },
    },
  };
});

const CLIENT_ROOT = fileURLToPath(new URL('.', import.meta.url));

const parseOrigin = (value: string | undefined): string | null => {
  try {
    return new URL(value ?? '').origin;
  } catch {
    return null;
  }
};
