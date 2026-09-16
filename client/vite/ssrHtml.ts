import type { ServerResponse } from 'node:http';
import type { Connect, Plugin } from 'vite';

/**
 * Routes document requests to the Rust server, which answers with the SPA
 * shell. Mirrors what the reverse proxy does in production, so anything the
 * server injects into the HTML is visible in dev too.
 *
 * @param serverOrigin Origin of the Rust server, without the `/api` prefix.
 */
export const ssrHtml = (serverOrigin: string): Plugin => ({
  name: 'tengri:ssr-html',
  apply: 'serve',
  // Registered here rather than in the returned post-hook so it runs *before*
  // Vite's own middlewares — its SPA fallback would otherwise answer first.
  // The cost is that every internal Vite path has to be excluded by hand.
  configureServer: (server) => {
    server.middlewares.use((req, res, next) => {
      if (!isDocumentRequest(req)) {
        next();
        return;
      }

      forwardToServer(serverOrigin, req, res, next);
    });
  },
});

const isDocumentRequest = (req: Connect.IncomingMessage): boolean => {
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    return false;
  }

  if (!req.headers.accept?.includes('text/html')) {
    return false;
  }

  const [path] = (req.url ?? '').split('?');
  // `/@vite/client`, `/@react-refresh` and `/src/App.tsx` reach this
  // middleware untransformed, and the first two have no extension to catch.
  if (
    path.startsWith('/api') ||
    path.startsWith('/@') || // custom Vite paths like @fs/…
    path.startsWith('/src/') ||
    path.startsWith('/node_modules/')
  ) {
    return false;
  }

  return !path.slice(path.lastIndexOf('/') + 1).includes('.');
};

const forwardToServer = async (
  serverOrigin: string,
  req: Connect.IncomingMessage,
  res: ServerResponse,
  next: Connect.NextFunction,
): Promise<void> => {
  let upstream: Response;
  try {
    upstream = await fetch(`${serverOrigin}${req.url}`, {
      headers: { accept: 'text/html' },
    });
  } catch {
    // Server is down: fall through to Vite's own index.html so frontend-only
    // work doesn't stall. A reachable server that errors is passed through
    // instead, since that's a misconfiguration worth seeing.
    next();
    return;
  }

  res.statusCode = upstream.status;
  res.setHeader(
    'content-type',
    upstream.headers.get('content-type') ?? 'text/html; charset=utf-8',
  );
  res.end(await upstream.text());
};
