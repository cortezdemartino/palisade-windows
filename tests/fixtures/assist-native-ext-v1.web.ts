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

export type ExtOp = keyof typeof REQUESTS;
export const EXT_SAFETY_OPS: readonly ExtOp[] = ['resource.cancel'];

const RESULTS: Record<ExtOp, Record<string, (v: unknown) => boolean>> = {
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

export type ExtEventName = keyof typeof EVENTS;

export interface ExtRequest {
  p: typeof EXT_PROTOCOL;
  v: 1;
  id: string;
  op: ExtOp;
  [field: string]: unknown;
}
export interface ExtEvent {
  p: typeof EXT_PROTOCOL;
  v: 1;
  event: ExtEventName;
  seq: number;
  [field: string]: unknown;
}
export type ExtResponse =
  | { p: typeof EXT_PROTOCOL; v: 1; id: string; ok: true; result: Rec }
  | { p: typeof EXT_PROTOCOL; v: 1; id: string; ok: false; error: ExtErrorCode };

function json(raw: unknown): unknown {
  if (typeof raw !== 'string') return raw;
  if (raw.length > NATIVE_LIMITS.messageChars) throw new ExtBridgeError('invalid-message');
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    throw new ExtBridgeError('invalid-message');
  }
}
function envelope(m: Rec): void {
  if (m.p !== EXT_PROTOCOL) throw new ExtBridgeError('invalid-message');
  if (m.v !== EXT_VERSION) throw new ExtBridgeError('unsupported-version');
}

export function parseExtRequest(raw: unknown): ExtRequest {
  const m = json(raw);
  if (!plain(m)) throw new ExtBridgeError('invalid-message');
  envelope(m);
  if (
    typeof m.id !== 'string' ||
    !REQUEST_ID.test(m.id) ||
    typeof m.op !== 'string' ||
    !Object.hasOwn(REQUESTS, m.op)
  ) {
    throw new ExtBridgeError('invalid-message');
  }
  const fields = REQUESTS[m.op as ExtOp] as Record<string, (v: unknown) => boolean>;
  if (!exact(m, ['p', 'v', 'id', 'op', ...Object.keys(fields)])) {
    throw new ExtBridgeError('invalid-message');
  }
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m[k])) throw new ExtBridgeError('invalid-message');
  }
  return m as ExtRequest;
}

export function parseExtResponse(raw: unknown, op: ExtOp): ExtResponse {
  const m = json(raw);
  if (!plain(m)) throw new ExtBridgeError('invalid-message');
  envelope(m);
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
  const fields = RESULTS[op];
  if (!exact(m.result, Object.keys(fields))) throw new ExtBridgeError('invalid-message');
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m.result[k])) throw new ExtBridgeError('invalid-message');
  }
  return m as ExtResponse;
}

export function parseExtEvent(raw: unknown): ExtEvent {
  const m = json(raw);
  if (!plain(m)) throw new ExtBridgeError('invalid-message');
  envelope(m);
  if (typeof m.event !== 'string' || !Object.hasOwn(EVENTS, m.event)) {
    throw new ExtBridgeError('invalid-message');
  }
  if (typeof m.seq !== 'number' || !Number.isSafeInteger(m.seq) || m.seq < 1) {
    throw new ExtBridgeError('invalid-message');
  }
  const fields = EVENTS[m.event as ExtEventName] as Record<string, (v: unknown) => boolean>;
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
