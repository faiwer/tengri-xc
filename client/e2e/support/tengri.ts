import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const repoRoot = path.resolve(
  fileURLToPath(new URL('../../..', import.meta.url)),
);

interface TengriOptions {
  /** Fed to the child's stdin, for flags like `user add --password-stdin`. */
  stdin?: string;
}

export function tengri(
  args: string[],
  options: TengriOptions = {},
): Promise<{ stdout: string; stderr: string }> {
  const child = spawn(
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
    },
  );

  // Closed either way: a subcommand that reads stdin hangs on an open pipe
  // with nothing coming.
  child.stdin.end(options.stdin ?? '');

  let stdout = '';
  let stderr = '';
  child.stdout.setEncoding('utf8').on('data', (chunk: string) => {
    stdout += chunk;
  });
  child.stderr.setEncoding('utf8').on('data', (chunk: string) => {
    stderr += chunk;
  });

  return new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        reject(
          new Error(`tengri ${args.join(' ')} exited with ${code}\n${stderr}`),
        );
      }
    });
  });
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
