/// <reference types="vitest" />
import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: './',
  plugins: [react()],
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
});
