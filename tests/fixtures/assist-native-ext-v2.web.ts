import { NATIVE_LIMITS } from './protocol';

/**
 * Native Assist bridge extension, `palisade.assist.native.ext` version 1
 * (docs/50-ASSIST-NATIVE-RESOURCES.md).
 *
 * Negotiated separately over the same two-member surface as `palisade.assist.native` v1 (docs/49),
 * whose messages, validator and fixtures stay unchanged. It adds the publisher endpoint's native
 * resource **local actions** (send clipboard text, offer a file — each confirmed natively), the
 * cancellation of a resource transfer, display-only resource status, and cleanup reporting that
 * keeps "authority revoked" separate from "native cleanup finished". The page never receives
 * clipboard contents, file names, paths or file data, and never approves anything.
 */

export const EXT_PROTOCOL = 'palisade.assist.native.ext';
export const EXT_VERSION = 1;
/**
 * Version 2 (docs/51 §3): image clipboard, temporary directional clipboard sync, several files at
 * once, the endpoint's own drop surface and verified transfer resume. Negotiated with the same
 * `ext.hello` (always a version 1 envelope); after that every request and event uses `v: 2`.
 * Version 1 messages, validator behaviour and fixtures are unchanged.
 */
export const EXT_VERSION_2 = 2;
export type ExtVersion = 1 | 2;

export const EXT_ERRORS = [
  'unsupported-version',
  'invalid-message',
  'busy',
  'consent-denied',
  'grant-invalid',
  'not-active',
  'unsupported',
  'too-large',
  'cancelled',
  'stale-operation',
  'timeout',
  'stopped',
  'internal',
] as const;
export type ExtErrorCode = (typeof EXT_ERRORS)[number];

export class ExtBridgeError extends Error {
  constructor(readonly code: ExtErrorCode) {
    super(code);
    this.name = 'ExtBridgeError';
  }
}

const ID = /^[A-Za-z0-9_-]{8,128}$/;
const REQUEST_ID = /^[A-Za-z0-9_-]{8,64}$/;
type Rec = Record<string, unknown>;
function plain(v: unknown): v is Rec {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v) as unknown;
  return proto === Object.prototype || proto === null;
}
function exact(r: Rec, keys: readonly string[]): boolean {
  const own = Object.keys(r).sort();
  return own.length === keys.length && [...keys].sort().every((k, i) => own[i] === k);
}
const id = (v: unknown) => typeof v === 'string' && ID.test(v);

const REQUESTS = {
  'ext.hello': {
    versions: (v: unknown) =>
      Array.isArray(v) &&
      v.length >= 1 &&
      v.length <= 4 &&
      v.every((x) => Number.isSafeInteger(x) && (x as number) >= 1),
  },
  /**
   * Publisher: send this computer's clipboard text to the helper under a `clipboard_to_guest`
   * grant. The endpoint confirms natively, reads plain text only (≤ 64 KiB UTF-8) and sends it over
   * the native peer's `clipboard` channel. The page never sees the text.
   */
  'resource.send-text': { session: id, grantId: id },
  /**
   * Publisher: offer one file to the helper under a `file_to_guest` grant. The endpoint opens its
   * own file picker (no path from the page) and sends a `file-offer` over the `file` channel.
   */
  'resource.offer-file': { session: id, grantId: id },
  /** Cancels one resource transfer in either direction (a safety request). */
  'resource.cancel': { session: id, transfer: id },
} as const satisfies Record<string, Record<string, (v: unknown) => boolean>>;

const REQUESTS_V2 = {
  'resource.send-text': REQUESTS['resource.send-text'],
  'resource.offer-file': REQUESTS['resource.offer-file'],
  'resource.cancel': REQUESTS['resource.cancel'],
  /**
   * Publisher: send this computer's clipboard **image** (`clipboard_to_guest`). The endpoint
   * confirms natively, re-encodes it as PNG within the limits and sends `clipboard-image`.
   */
  'resource.send-image': { session: id, grantId: id },
  /** Publisher: native multi-select picker (≤ 32 files), each offered and accepted separately. */
  'resource.offer-files': { session: id, grantId: id },
  /**
   * Publisher: open the endpoint's own drop surface (an app-owned window). Dropped files are
   * confirmed natively per file before any offer; nothing dropped reaches the page.
   */
  'resource.drop-surface': { session: id, grantId: id },
  /** Publisher: resume an interrupted outgoing transfer (native confirmation, fresh grant). */
  'resource.resume': { session: id, transfer: id },
  /**
   * Publisher: turn temporary clipboard sync on (native consent) or off (a safety request).
   * Under `clipboard_to_guest` the endpoint sends this computer's clipboard text when it
   * changes; under `clipboard_to_host` it applies received text to this computer's clipboard.
   */
  'resource.sync': {
    session: id,
    grantId: id,
    state: (v: unknown) => v === 'on' || v === 'off',
  },
} as const satisfies Record<string, Record<string, (v: unknown) => boolean>>;

export type ExtOp = keyof typeof REQUESTS | keyof typeof REQUESTS_V2;
export const EXT_SAFETY_OPS: readonly ExtOp[] = ['resource.cancel'];

/** Stop/cancel requests use the separate safety budget (docs/50 §6); sync `off` is one. */
export function isExtSafety(op: ExtOp, fields: Record<string, unknown> = {}): boolean {
  return EXT_SAFETY_OPS.includes(op) || (op === 'resource.sync' && fields.state === 'off');
}

function requestTable(
  version: ExtVersion,
): Record<string, Record<string, (v: unknown) => boolean>> {
  return version === 2 ? REQUESTS_V2 : REQUESTS;
}

const RESULTS: Record<keyof typeof REQUESTS, Record<string, (v: unknown) => boolean>> = {
  'ext.hello': {
    version: (v) => v === EXT_VERSION,
    /** Descriptive only; never used to decide access. */
    features: (v) =>
      plain(v) &&
      exact(v, ['cleanupEvents', 'clipboardText', 'files']) &&
      Object.values(v).every((x) => typeof x === 'boolean'),
  },
  'resource.send-text': { transfer: id },
  'resource.offer-file': { transfer: id },
  'resource.cancel': {},
};

/** Version 2 features (descriptive only; never used to decide access). */
export const EXT_V2_FEATURES = [
  'cleanupEvents',
  'clipboardImage',
  'clipboardSync',
  'clipboardText',
  'dropSurface',
  'fileResume',
  'files',
  'input2',
  'multiFile',
] as const;
export type ExtV2Features = Record<(typeof EXT_V2_FEATURES)[number], boolean>;

const transferList = (v: unknown) =>
  Array.isArray(v) &&
  v.length >= 1 &&
  v.length <= 32 &&
  v.every(id) &&
  new Set(v as string[]).size === v.length;

const RESULTS_V2: Record<keyof typeof REQUESTS_V2, Record<string, (v: unknown) => boolean>> = {
  'resource.send-text': { transfer: id },
  'resource.offer-file': { transfer: id },
  'resource.cancel': {},
  'resource.send-image': { transfer: id },
  'resource.offer-files': { transfers: transferList },
  'resource.drop-surface': {},
  'resource.resume': { transfer: id },
  'resource.sync': {},
};

/** The `ext.hello` result: version 1 or 2, with that version's feature set. */
function helloResult(r: Rec): boolean {
  if (!exact(r, ['version', 'features']) || !plain(r.features)) return false;
  const keys =
    r.version === EXT_VERSION
      ? ['cleanupEvents', 'clipboardText', 'files']
      : r.version === EXT_VERSION_2
        ? [...EXT_V2_FEATURES]
        : undefined;
  return (
    keys !== undefined &&
    exact(r.features, keys) &&
    Object.values(r.features).every((x) => typeof x === 'boolean')
  );
}

export const RESOURCE_STATES = [
  'awaiting-local-confirmation',
  'offered',
  'awaiting-acceptance',
  'transferring',
  'verifying',
  'complete',
  'rejected',
  'cancelled',
  'failed',
] as const;
export type ResourceState = (typeof RESOURCE_STATES)[number];

const REASONS = [
  'local-denied',
  'peer-rejected',
  'grant-ended',
  'stream-ended',
  'integrity-failure',
  'too-large',
  'quota',
  'stalled',
  'local-stop',
  'error',
];

const EVENTS = {
  /** Display-only progress of one resource transfer; no names, paths or contents. */
  'resource.status': {
    session: id,
    transfer: id,
    kind: (v: unknown) => v === 'text' || v === 'file',
    direction: (v: unknown) => v === 'to-host' || v === 'to-guest',
    state: (v: unknown) => RESOURCE_STATES.includes(v as ResourceState),
    bytes: (v: unknown) => Number.isSafeInteger(v) && (v as number) >= 0,
    total: (v: unknown) => Number.isSafeInteger(v) && (v as number) >= 0,
    reason: (v: unknown) => v === null || (typeof v === 'string' && REASONS.includes(v)),
  },
  /**
   * Native cleanup after authority ended: `pending` while held keys/buttons, input children or
   * staged files are still being released; `complete` once nothing native remains. Authority is
   * already gone when `pending` is reported.
   */
  cleanup: {
    session: (v: unknown) => v === null || id(v),
    state: (v: unknown) => v === 'pending' || v === 'complete',
    reason: (v: unknown) =>
      v === null ||
      (typeof v === 'string' &&
        [
          'local-stop',
          'revoked',
          'expired',
          'stream-ended',
          'lease-lost',
          'quit',
          'update',
          'error',
        ].includes(v)),
  },
} as const satisfies Record<string, Record<string, (v: unknown) => boolean>>;

const SYNC_REASONS = ['local-stop', 'revoked', 'expired', 'stream-ended', 'lease-lost', 'error'];

const EVENTS_V2 = {
  /** Version 2 adds the resource kind `image` and whether an interruption can be resumed. */
  'resource.status': {
    ...EVENTS['resource.status'],
    kind: (v: unknown) => v === 'text' || v === 'image' || v === 'file',
    resumable: (v: unknown) => typeof v === 'boolean',
  },
  cleanup: EVENTS.cleanup,
  /** Temporary clipboard sync on this computer: on, or off and why. */
  'sync.status': {
    session: id,
    capability: (v: unknown) => v === 'clipboard_to_guest' || v === 'clipboard_to_host',
    state: (v: unknown) => v === 'on' || v === 'off',
    reason: (v: unknown) => v === null || (typeof v === 'string' && SYNC_REASONS.includes(v)),
  },
} as const satisfies Record<string, Record<string, (v: unknown) => boolean>>;

export type ExtEventName = keyof typeof EVENTS | keyof typeof EVENTS_V2;

export interface ExtRequest {
  p: typeof EXT_PROTOCOL;
  v: ExtVersion;
  id: string;
  op: ExtOp;
  [field: string]: unknown;
}
export interface ExtEvent {
  p: typeof EXT_PROTOCOL;
  v: ExtVersion;
  event: ExtEventName;
  seq: number;
  [field: string]: unknown;
}
export type ExtResponse =
  | { p: typeof EXT_PROTOCOL; v: ExtVersion; id: string; ok: true; result: Rec }
  | { p: typeof EXT_PROTOCOL; v: ExtVersion; id: string; ok: false; error: ExtErrorCode };

function json(raw: unknown): unknown {
  if (typeof raw !== 'string') return raw;
  if (raw.length > NATIVE_LIMITS.messageChars) throw new ExtBridgeError('invalid-message');
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    throw new ExtBridgeError('invalid-message');
  }
}
/**
 * The envelope version: the negotiated one, except `ext.hello` (and its answer), which always
 * uses version 1 because it happens before anything is negotiated.
 */
function envelope(m: Rec, version: ExtVersion, hello: boolean): void {
  if (m.p !== EXT_PROTOCOL) throw new ExtBridgeError('invalid-message');
  if (m.v !== (hello ? EXT_VERSION : version)) throw new ExtBridgeError('unsupported-version');
}

/** Parses a request under the negotiated version (default 1, exactly as before). */
export function parseExtRequest(raw: unknown, version: ExtVersion = EXT_VERSION): ExtRequest {
  const m = json(raw);
  if (!plain(m)) throw new ExtBridgeError('invalid-message');
  envelope(m, version, m.op === 'ext.hello');
  const table: Record<string, Record<string, (v: unknown) => boolean>> = m.op === 'ext.hello'
    ? REQUESTS
    : requestTable(version);
  if (
    typeof m.id !== 'string' ||
    !REQUEST_ID.test(m.id) ||
    typeof m.op !== 'string' ||
    !Object.hasOwn(table, m.op)
  ) {
    throw new ExtBridgeError('invalid-message');
  }
  const fields = table[m.op] as Record<string, (v: unknown) => boolean>;
  if (!exact(m, ['p', 'v', 'id', 'op', ...Object.keys(fields)])) {
    throw new ExtBridgeError('invalid-message');
  }
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m[k])) throw new ExtBridgeError('invalid-message');
  }
  return m as ExtRequest;
}

export function parseExtResponse(
  raw: unknown,
  op: ExtOp,
  version: ExtVersion = EXT_VERSION,
): ExtResponse {
  const m = json(raw);
  if (!plain(m)) throw new ExtBridgeError('invalid-message');
  envelope(m, version, op === 'ext.hello');
  if (typeof m.id !== 'string' || !REQUEST_ID.test(m.id))
    throw new ExtBridgeError('invalid-message');
  if (m.ok === false) {
    if (
      !exact(m, ['p', 'v', 'id', 'ok', 'error']) ||
      !EXT_ERRORS.includes(m.error as ExtErrorCode)
    ) {
      throw new ExtBridgeError('invalid-message');
    }
    return m as ExtResponse;
  }
  if (m.ok !== true || !exact(m, ['p', 'v', 'id', 'ok', 'result']) || !plain(m.result)) {
    throw new ExtBridgeError('invalid-message');
  }
  if (op === 'ext.hello' && version === EXT_VERSION_2) {
    // A client offering both versions accepts either answer, each with its own features.
    if (!helloResult(m.result)) throw new ExtBridgeError('invalid-message');
    return m as ExtResponse;
  }
  const table = (version === 2 && op !== 'ext.hello' ? RESULTS_V2 : RESULTS) as Record<
    string,
    Record<string, (v: unknown) => boolean> | undefined
  >;
  const fields = table[op];
  if (!fields) throw new ExtBridgeError('invalid-message');
  if (!exact(m.result, Object.keys(fields))) throw new ExtBridgeError('invalid-message');
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m.result[k])) throw new ExtBridgeError('invalid-message');
  }
  return m as ExtResponse;
}

export function parseExtEvent(raw: unknown, version: ExtVersion = EXT_VERSION): ExtEvent {
  const m = json(raw);
  if (!plain(m)) throw new ExtBridgeError('invalid-message');
  envelope(m, version, false);
  const events = (version === 2 ? EVENTS_V2 : EVENTS) as Record<
    string,
    Record<string, (v: unknown) => boolean>
  >;
  if (typeof m.event !== 'string' || !Object.hasOwn(events, m.event)) {
    throw new ExtBridgeError('invalid-message');
  }
  if (typeof m.seq !== 'number' || !Number.isSafeInteger(m.seq) || m.seq < 1) {
    throw new ExtBridgeError('invalid-message');
  }
  const fields = events[m.event] as Record<string, (v: unknown) => boolean>;
  if (!exact(m, ['p', 'v', 'event', 'seq', ...Object.keys(fields)])) {
    throw new ExtBridgeError('invalid-message');
  }
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m[k])) throw new ExtBridgeError('invalid-message');
  }
  return m as ExtEvent;
}

/** Which protocol a message on the shared surface belongs to, by its `p` field. */
export function protocolOf(raw: string): 'native' | 'ext' | undefined {
  try {
    const m = JSON.parse(raw) as unknown;
    if (!plain(m)) return undefined;
    return m.p === EXT_PROTOCOL ? 'ext' : m.p === 'palisade.assist.native' ? 'native' : undefined;
  } catch {
    return undefined;
  }
}
