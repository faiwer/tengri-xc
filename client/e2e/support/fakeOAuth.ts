import {
  createServer,
  type IncomingMessage,
  type Server,
  type ServerResponse,
} from 'node:http';

/** Port the stand-in listens on; the server's `OAUTH_ENDPOINT_BASE` points here. */
export const OAUTH_PORT = Number(process.env.E2E_OAUTH_PORT ?? 1080);

export const getOAuthEndpointBase = (): string =>
  `http://127.0.0.1:${OAUTH_PORT}`;

/** What userinfo hands back, in the shape Google's OIDC response has. */
export interface FakeIdentity {
  /** Stable provider-side id — what a link is keyed on. */
  sub: string;
  email?: string;
  /** The server drops an address the provider won't vouch for. */
  emailVerified?: boolean;
  name?: string;
}

export interface FakeOAuth {
  /** Who the next authorize hop consents as. */
  signInAs(identity: FakeIdentity): void;
  /**
   * Refuse to redeem authorization codes from here on, the way an outage or a
   * rotated client secret does. Consent still succeeds, so the browser is
   * already back at our callback when the flow falls over.
   */
  breakTokenExchange(): void;
  /** Forget the identity and any codes/tokens still outstanding. */
  reset(): void;
  close(): Promise<void>;
}

/**
 * A provider that always says yes, standing in for the real endpoints under
 * `{base}/{provider}/authorize|token|userinfo`.
 *
 * `/authorize` never renders a consent screen — it bounces straight back to
 * the callback — so a flow is one uninterrupted chain of redirects from the
 * browser's point of view. The identity is captured when that hop happens, so
 * a test sets it before clicking and each flow keeps the one it started with.
 */
export async function startFakeOAuth(port = OAUTH_PORT): Promise<FakeOAuth> {
  const state: ProviderState = {
    identity: null,
    tokenExchangeBroken: false,
    codes: new Map(),
    tokens: new Map(),
  };

  const server = createServer((request, response) => {
    route(request, response, state).catch((err: unknown) => {
      sendJson(response, 500, { error: String(err) });
    });
  });
  await listen(server, port);

  return {
    signInAs: (identity) => {
      state.identity = identity;
    },
    breakTokenExchange: () => {
      state.tokenExchangeBroken = true;
    },
    reset: () => {
      state.identity = null;
      state.tokenExchangeBroken = false;
      state.codes.clear();
      state.tokens.clear();
    },
    close: () =>
      new Promise((resolve, reject) => {
        server.close((err) => (err ? reject(err) : resolve()));
      }),
  };
}

interface ProviderState {
  identity: FakeIdentity | null;
  tokenExchangeBroken: boolean;
  codes: Map<string, FakeIdentity>;
  tokens: Map<string, FakeIdentity>;
}

async function route(
  request: IncomingMessage,
  response: ServerResponse,
  state: ProviderState,
): Promise<void> {
  const url = new URL(request.url ?? '/', getOAuthEndpointBase());

  switch (url.pathname.split('/').pop()) {
    case 'authorize':
      return authorize(url, response, state);
    case 'token':
      return exchangeCode(await readBody(request), response, state);
    case 'userinfo':
      return describeUser(request, response, state);
    default:
      return sendJson(response, 404, { error: `no route for ${url.pathname}` });
  }
}

function authorize(
  url: URL,
  response: ServerResponse,
  state: ProviderState,
): void {
  const { identity } = state;
  if (!identity) {
    return sendJson(response, 500, {
      error: 'no identity; call signInAs() before starting a flow',
    });
  }

  const redirectUri = url.searchParams.get('redirect_uri');
  if (!redirectUri) {
    return sendJson(response, 400, { error: 'redirect_uri is required' });
  }

  const code = makeToken();
  state.codes.set(code, identity);

  // The state param is echoed untouched: the server matches it against its
  // flow cookie, and swallowing it would look like a CSRF attempt.
  const callback = new URL(redirectUri);
  callback.searchParams.set('code', code);
  callback.searchParams.set('state', url.searchParams.get('state') ?? '');

  response.writeHead(302, { location: callback.toString() });
  response.end();
}

function exchangeCode(
  body: string,
  response: ServerResponse,
  state: ProviderState,
): void {
  if (state.tokenExchangeBroken) {
    return sendJson(response, 503, { error: 'temporarily_unavailable' });
  }

  const code = new URLSearchParams(body).get('code') ?? '';
  const identity = state.codes.get(code);
  if (!identity) {
    return sendJson(response, 400, { error: 'invalid_grant' });
  }
  // Codes are single-use for real providers, and a replay here would mean the
  // callback ran twice.
  state.codes.delete(code);

  const accessToken = makeToken();
  state.tokens.set(accessToken, identity);
  sendJson(response, 200, {
    access_token: accessToken,
    token_type: 'Bearer',
    expires_in: 3600,
  });
}

function describeUser(
  request: IncomingMessage,
  response: ServerResponse,
  state: ProviderState,
): void {
  const bearer = (request.headers.authorization ?? '').replace(/^Bearer /i, '');
  const identity = state.tokens.get(bearer);
  if (!identity) {
    return sendJson(response, 401, { error: 'invalid_token' });
  }

  sendJson(response, 200, {
    sub: identity.sub,
    name: identity.name,
    email: identity.email,
    email_verified: identity.emailVerified,
  });
}

const listen = (server: Server, port: number): Promise<void> =>
  new Promise((resolve, reject) => {
    const onListenError = (err: Error) => reject(err);
    server.once('error', onListenError);
    server.listen(port, '127.0.0.1', () => {
      server.off('error', onListenError);
      server.on('error', (err) => console.error('fake OAuth provider:', err));
      resolve();
    });
  });

async function readBody(request: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(chunk as Buffer);
  }
  return Buffer.concat(chunks).toString('utf8');
}

function sendJson(
  response: ServerResponse,
  status: number,
  body: unknown,
): void {
  response.writeHead(status, { 'content-type': 'application/json' });
  response.end(JSON.stringify(body));
}

const makeToken = (): string => crypto.randomUUID().replaceAll('-', '');
