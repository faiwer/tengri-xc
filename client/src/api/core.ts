import type { z } from 'zod';

const SERVER_URL = import.meta.env.VITE_SERVER_URL;

/** Base class so callers can `catch (e) { if (e instanceof ApiError) ... }`. */
export class ApiError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = new.target.name;
  }
}

/** Network-level failure: DNS, offline, CORS, fetch threw, etc. */
export class NetworkError extends ApiError {}

/** Server returned a non-2xx response. */
export class HttpError extends ApiError {
  readonly status: number;

  constructor(status: number, message?: string) {
    super(message ?? `HTTP ${status}`);
    this.status = status;
  }
}

/** Server returned a 2xx but the body did not match the expected schema. */
export class DecodeError extends ApiError {
  readonly issues: z.core.$ZodIssue[];
  readonly raw: unknown;

  constructor(issues: z.core.$ZodIssue[], raw: unknown) {
    super(`Response did not match schema: ${formatIssues(issues)}`);
    this.issues = issues;
    this.raw = raw;
  }
}

function formatIssues(issues: z.core.$ZodIssue[]): string {
  return issues
    .map((i) => `${i.path.join('.') || '<root>'}: ${i.message}`)
    .join('; ');
}

/** Issue the GET, normalize transport/HTTP failures into `ApiError` subclasses. */
async function fetchOk(path: string): Promise<Response> {
  const url = SERVER_URL + path;
  let response: Response;
  try {
    response = await fetch(url, { method: 'GET' });
  } catch (cause) {
    throw new NetworkError(`GET ${url} failed`, { cause });
  }
  if (!response.ok) {
    throw new HttpError(response.status);
  }
  return response;
}

/**
 * GET `path` (resolved against `VITE_SERVER_URL`) and validate the JSON body
 * against `schema`. Returns the decoded value on success; throws an
 * `ApiError` subclass otherwise.
 */
export async function apiGet<T extends z.ZodTypeAny>(
  path: string,
  schema: T,
): Promise<z.infer<T>> {
  const response = await fetchOk(path);
  let body: unknown;
  try {
    body = await response.json();
  } catch (cause) {
    throw new DecodeError([], cause);
  }
  const parsed = schema.safeParse(body);
  if (!parsed.success) {
    throw new DecodeError(parsed.error.issues, body);
  }
  return parsed.data;
}

/**
 * GET `path` and return the raw response body as a `Blob`. The browser
 * transparently honors `Content-Encoding`, so a gzipped response is already
 * decompressed by the time it reaches the Blob.
 */
export async function apiGetBlob(path: string): Promise<Blob> {
  const response = await fetchOk(path);
  return response.blob();
}
