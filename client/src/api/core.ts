import camelcaseKeys from 'camelcase-keys';
import snakecaseKeys from 'snakecase-keys';
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

/**
 * 422 with a `{ fields: { … } }` body. Carries the per-field messages
 * keyed by the server's dotted path (e.g. `'profile.country'`) so a
 * form can drive `Form.setFields` directly without re-deriving the
 * shape from the human message.
 *
 * Field paths arrive snake_case from the server (`'profile.civl_id'`)
 * and are camelized at this boundary (`'profile.civlId'`) so the FE
 * keys match `Form.Item name={['profile', 'civlId']}` without
 * per-call-site translation.
 */
export class ValidationError extends HttpError {
  readonly fields: Record<string, string>;

  constructor(fields: Record<string, string>, message?: string) {
    super(422, message ?? 'Validation failed');
    this.fields = fields;
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

export interface ApiRequestOptions {
  signal?: AbortSignal;
}

interface FetchOptions extends ApiRequestOptions {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  body?: unknown;
}

/** Issue the request, normalize transport/HTTP failures into `ApiError` subclasses. */
async function fetchOk(path: string, options: FetchOptions): Promise<Response> {
  const url = SERVER_URL + path;
  const method = options.method ?? 'GET';
  const init: RequestInit = {
    method,
    signal: options.signal,
    // `include` so the browser ships the session cookie cross-origin
    // (Vite dev :5173 → API :5757). The server matches by listing the
    // dev origin in `CLIENT_ORIGINS`; without that, this header is a
    // no-op.
    credentials: 'include',
  };
  if (options.body !== undefined) {
    init.headers = { 'Content-Type': 'application/json' };
    // Mirror image of the inbound camelcase step: the wire is
    // snake_case, so transform the body once at this boundary.
    const wireBody =
      options.body !== null && typeof options.body === 'object'
        ? snakecaseKeys(options.body as Record<string, unknown>, { deep: true })
        : options.body;
    init.body = JSON.stringify(wireBody);
  }

  let response: Response;
  try {
    response = await fetch(url, init);
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === 'AbortError') {
      throw cause;
    }

    throw new NetworkError(`${method} ${url} failed`, { cause });
  }
  if (!response.ok) {
    throw await readErrorBody(response);
  }
  return response;
}

/**
 * Build the right `ApiError` subclass for a non-OK response. 422 with
 * a `{ fields }` body becomes [`ValidationError`]; everything else
 * stays a plain [`HttpError`]. Body parsing failures fall back to a
 * status-only `HttpError` (the request did fail; the failure shape
 * is just opaque to the caller).
 */
async function readErrorBody(response: Response): Promise<HttpError> {
  if (response.status !== 422) {
    return new HttpError(response.status);
  }

  let raw: unknown;
  try {
    raw = await response.json();
  } catch {
    return new HttpError(422);
  }

  // Same camelcase rewrite as success bodies: server keys are
  // snake_case (`profile.civl_id`), the FE wants camelCase tail
  // segments (`profile.civlId`) so they match `Form.Item name={…}`.
  const camelized =
    raw !== null && typeof raw === 'object'
      ? (camelcaseKeys(raw as Record<string, unknown>, { deep: true }) as {
          message?: string;
          fields?: Record<string, string>;
        })
      : null;
  const fields = camelized?.fields;
  if (!fields || typeof fields !== 'object') {
    return new HttpError(422, camelized?.message);
  }

  return new ValidationError(fields, camelized?.message);
}

/**
 * GET `path` (resolved against `VITE_SERVER_URL`) and validate the JSON body
 * against `schema`. Returns the decoded value on success; throws an
 * `ApiError` subclass otherwise. Pass `options.signal` to make the call
 * abortable.
 */
export async function apiGet<T extends z.ZodTypeAny>(
  path: string,
  schema: T,
  options: ApiRequestOptions = {},
): Promise<z.infer<T>> {
  const response = await fetchOk(path, options);
  return decodeJson(response, schema);
}

/**
 * POST JSON `body` to `path` and validate the response against `schema`.
 * The body is sent as `application/json`. Errors mirror `apiGet`.
 */
export async function apiPost<T extends z.ZodTypeAny>(
  path: string,
  body: unknown,
  schema: T,
  options: ApiRequestOptions = {},
): Promise<z.infer<T>> {
  const response = await fetchOk(path, { ...options, method: 'POST', body });
  return decodeJson(response, schema);
}

/**
 * POST JSON `body` to `path` and ignore the response body. Use for
 * 204-style endpoints (e.g. logout).
 */
export async function apiPostVoid(
  path: string,
  body: unknown = null,
  options: ApiRequestOptions = {},
): Promise<void> {
  await fetchOk(path, { ...options, method: 'POST', body });
}

/**
 * PATCH JSON `body` to `path` and validate the response against
 * `schema`. Errors mirror `apiPost`; 422s with a `fields` body land
 * as [`ValidationError`].
 */
export async function apiPatch<T extends z.ZodTypeAny>(
  path: string,
  body: unknown,
  schema: T,
  options: ApiRequestOptions = {},
): Promise<z.infer<T>> {
  const response = await fetchOk(path, { ...options, method: 'PATCH', body });
  return decodeJson(response, schema);
}

/**
 * GET `path` and return the raw response body as a `Blob`. The browser
 * transparently honors `Content-Encoding`, so a gzipped response is already
 * decompressed by the time it reaches the Blob.
 */
export async function apiGetBlob(
  path: string,
  options: ApiRequestOptions = {},
): Promise<Blob> {
  const response = await fetchOk(path, options);
  return response.blob();
}

async function decodeJson<T extends z.ZodTypeAny>(
  response: Response,
  schema: T,
): Promise<z.infer<T>> {
  let raw: unknown;
  try {
    raw = await response.json();
  } catch (cause) {
    throw new DecodeError([], cause);
  }

  // The wire is snake_case (Rust convention); the rest of the client
  // is camelCase. Convert at this single boundary so schemas and
  // consumers never see a snake_case key.
  const body =
    raw !== null && typeof raw === 'object'
      ? camelcaseKeys(raw as Record<string, unknown>, { deep: true })
      : raw;
  const parsed = schema.safeParse(body);
  if (!parsed.success) {
    throw new DecodeError(parsed.error.issues, body);
  }
  return parsed.data;
}
