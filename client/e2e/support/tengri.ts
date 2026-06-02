import { execFile } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

export const repoRoot = path.resolve(
  fileURLToPath(new URL('../../..', import.meta.url)),
);

export async function tengri(
  args: string[],
): Promise<{ stdout: string; stderr: string }> {
  return execFileAsync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      path.join(repoRoot, 'server/Cargo.toml'),
      '--bin',
      'tengri',
      '--',
      ...args,
    ],
    {
      cwd: repoRoot,
      env: { ...process.env, DATABASE_URL: e2eDatabaseUrl() },
      maxBuffer: 10 * 1024 * 1024,
    },
  );
}

export function e2eDatabaseUrl(): string {
  return requiredEnv('E2E_DATABASE_URL');
}

export function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required for E2E tests`);
  }
  return value;
}
