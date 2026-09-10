import { createServer, type Server, type Socket } from 'node:net';

/** Port the sink listens on. The stored SMTP settings have to point here. */
export const SMTP_PORT = Number(process.env.E2E_SMTP_PORT ?? 1025);

/** One delivered message, with its transfer encoding already undone. */
export interface Mail {
  /** Envelope sender (`MAIL FROM`), not the `From:` header. */
  from: string;
  /** Envelope recipients (`RCPT TO`), lowercased. */
  to: string[];
  subject: string;
  /** Value of the `Content-Type` header, verbatim. */
  contentType: string;
  /**
   * Body with its transfer encoding undone. Not interpreted further — a
   * multipart message would land here boundaries and all.
   */
  body: string;
}

/** Reads over the messages one sink has collected. */
export interface Mailbox {
  /**
   * Wait for a message addressed to `recipient`, then remove it. Rejects when
   * nothing arrives in time, naming whatever else is sitting in the box.
   */
  take(recipient: string, timeoutMs?: number): Promise<Mail>;
  /** Messages still waiting for `recipient` — for "and nothing else" checks. */
  countFor(recipient: string): number;
  clear(): void;
}

export interface FakeSmtp {
  mailbox: Mailbox;
  close(): Promise<void>;
}

/**
 * Listen on `port` and keep whatever gets delivered. Speaks the sliver of SMTP
 * `lettre` uses once the transport is built with no TLS and no credentials:
 * EHLO, MAIL, RCPT, DATA, QUIT, and no extensions advertised.
 */
export async function startFakeSmtp(port = SMTP_PORT): Promise<FakeSmtp> {
  const delivered: Mail[] = [];
  const open = new Set<Socket>();
  const server = createServer((socket) => {
    // called once per each connection
    open.add(socket);
    socket.on('close', () => open.delete(socket));
    serveSocket(socket, delivered);
  });

  await listen(server, port);

  return {
    mailbox: createMailbox(delivered),
    close: async () => {
      // `server.close` only stops accepting, so a client that never sent QUIT
      // would hold the worker open past teardown.
      for (const socket of open) {
        socket.destroy();
      }
      await new Promise<void>((resolve, reject) => {
        server.close((err) => (err ? reject(err) : resolve()));
      });
    },
  };
}

const listen = (server: Server, port: number): Promise<void> =>
  new Promise((resolve, reject) => {
    const onListenError = (err: Error) => reject(err);
    server.once('error', onListenError);
    server.listen(port, '127.0.0.1', () => {
      server.off('error', onListenError);
      server.on('error', (err) => console.error('fake SMTP server:', err));
      resolve();
    });
  });

function serveSocket(socket: Socket, delivered: Mail[]): void {
  let envelope = emptyEnvelope();
  let inData = false;
  let buffer = '';

  // latin1 keeps one byte per char, so the DATA terminator search can't trip
  // over a multi-byte sequence. Bodies are decoded to UTF-8 in `parseMail`.
  socket.setEncoding('latin1');
  socket.write(`220 tengri-e2e ESMTP${CRLF}`);

  socket.on('data', (chunk: string) => {
    buffer += chunk;

    for (;;) {
      if (inData) {
        const end = buffer.indexOf(END_OF_DATA);
        if (end < 0) {
          break;
        }
        delivered.push(parseMail(buffer.slice(0, end), envelope));
        buffer = buffer.slice(end + END_OF_DATA.length);
        inData = false;
        envelope = emptyEnvelope();
        socket.write(`250 Queued${CRLF}`);
        continue;
      }

      const eol = buffer.indexOf(CRLF);
      if (eol < 0) {
        break;
      }
      const line = buffer.slice(0, eol);
      buffer = buffer.slice(eol + CRLF.length);

      const reply = handleCommand(line, envelope);
      socket.write(reply.text + CRLF);
      if (reply.enterData) {
        inData = true;
      }
      if (reply.reset) {
        envelope = emptyEnvelope();
      }
      if (reply.quit) {
        socket.end();
        return;
      }
    }
  });

  socket.on('error', () => socket.destroy());
}

interface Reply {
  text: string;
  enterData?: boolean;
  reset?: boolean;
  quit?: boolean;
}

function handleCommand(line: string, envelope: Envelope): Reply {
  const [verb, ...rest] = line.split(' ');

  switch (verb.toUpperCase()) {
    case 'EHLO':
    case 'HELO':
      // A single-line greeting advertises no extensions, which is what keeps
      // the client off STARTTLS, AUTH, and SMTPUTF8.
      return { text: '250 tengri-e2e' };
    case 'MAIL':
      envelope.from = parseAddress(rest.join(' '));
      return { text: '250 OK' };
    case 'RCPT':
      envelope.to.push(parseAddress(rest.join(' ')));
      return { text: '250 OK' };
    case 'DATA':
      return { text: '354 End data with <CR><LF>.<CR><LF>', enterData: true };
    case 'RSET':
      return { text: '250 OK', reset: true };
    case 'NOOP':
      return { text: '250 OK' };
    case 'QUIT':
      return { text: '221 Bye', quit: true };
    default:
      return { text: `502 ${verb} not implemented` };
  }
}

interface Envelope {
  from: string;
  to: string[];
}

const emptyEnvelope = (): Envelope => ({ from: '', to: [] });

/** `FROM:<pilot@example.test>` and friends down to the bare address. */
const parseAddress = (raw: string): string => {
  const match = /<([^>]*)>/.exec(raw);
  return (match ? match[1] : raw).trim().toLowerCase();
};

function parseMail(wire: string, envelope: Envelope): Mail {
  const raw = wire.replace(/^\.\./gm, '.');
  const split = /\r?\n\r?\n/.exec(raw);
  const headers = parseHeaders(split ? raw.slice(0, split.index) : '');
  const body = split ? raw.slice(split.index + split[0].length) : raw;

  return {
    from: envelope.from,
    to: [...envelope.to],
    subject: headers.get('subject') ?? '',
    contentType: headers.get('content-type') ?? '',
    body: decodeBody(body, headers.get('content-transfer-encoding')),
  };
}

function parseHeaders(raw: string): Map<string, string> {
  const headers = new Map<string, string>();

  for (const line of raw.replace(/\r?\n[ \t]+/g, ' ').split(/\r?\n/)) {
    const colon = line.indexOf(':');
    if (colon > 0) {
      headers.set(
        line.slice(0, colon).toLowerCase(),
        line.slice(colon + 1).trim(),
      );
    }
  }

  return headers;
}

function decodeBody(body: string, encoding: string | undefined): string {
  switch (encoding?.toLowerCase()) {
    case 'base64':
      return Buffer.from(body, 'base64').toString('utf8');
    case 'quoted-printable':
      return latin1ToUtf8(decodeQuotedPrintable(body));
    default:
      return latin1ToUtf8(body);
  }
}

/**
 * `lettre` picks quoted-printable for any body with a line of 76 bytes or
 * more, which ours always have — so without this the confirmation link arrives
 * as `?token=3D…` chopped up by soft `=\r\n` breaks.
 */
const decodeQuotedPrintable = (body: string): string =>
  body
    .replace(/=\r?\n/g, '')
    .replace(/=([0-9A-Fa-f]{2})/g, (_, hex: string) =>
      String.fromCharCode(parseInt(hex, 16)),
    );

const latin1ToUtf8 = (raw: string): string =>
  Buffer.from(raw, 'latin1').toString('utf8');

function createMailbox(delivered: Mail[]): Mailbox {
  const isAddressedTo = (mail: Mail, recipient: string) =>
    mail.to.includes(recipient.toLowerCase());

  return {
    take: async (recipient, timeoutMs = TAKE_TIMEOUT_MS) => {
      const deadline = Date.now() + timeoutMs;

      for (;;) {
        const index = delivered.findIndex((mail) =>
          isAddressedTo(mail, recipient),
        );
        if (index >= 0) {
          return delivered.splice(index, 1)[0];
        }
        if (Date.now() >= deadline) {
          throw new Error(
            `No mail for ${recipient} within ${timeoutMs}ms. ` +
              `Mailbox holds: ${summarize(delivered)}`,
          );
        }
        await sleep(POLL_MS);
      }
    },

    countFor: (recipient) =>
      delivered.filter((mail) => isAddressedTo(mail, recipient)).length,

    clear: () => {
      delivered.length = 0;
    },
  };
}

const summarize = (delivered: Mail[]): string =>
  delivered.length === 0
    ? 'nothing'
    : delivered
        .map((mail) => `${mail.to.join(', ')} — ${mail.subject}`)
        .join('; ');

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

const CRLF = '\r\n';
const END_OF_DATA = '\r\n.\r\n';
const TAKE_TIMEOUT_MS = 5_000;
const POLL_MS = 25;
