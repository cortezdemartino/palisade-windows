/**
 * Native Assist bridge, `palisade.assist.native` version 1 (docs/49-ASSIST-NATIVE-BRIDGE.md).
 *
 * The typed, separately negotiated channel between the hosted page and a Palisade desktop
 * endpoint. It is not the description bridge (`palisade.assist.bridge` v1, docs/45), which stays
 * unchanged. Every operation is one of a fixed list with an exact shape: there is no generic
 * invoke, no path, no command, no key export and no Matrix credential. Nothing the page sends is
 * authority: the endpoint verifies service-signed grants and leases itself, asks for local consent
 * itself, and checks the helper's certificate on its own peer connection.
 */

export const NATIVE_PROTOCOL = 'palisade.assist.native';
export const NATIVE_VERSION = 1;

export const NATIVE_LIMITS = Object.freeze({
  messageChars: 32_768,
  sdpChars: 16_384,
  candidateChars: 1_024,
  grantChars: 4_096,
  leaseChars: 2_048,
  /** Requests a page may have outstanding at once. */
  outstanding: 8,
  /** A request with no response by then is failed by the page (`timeout`). */
  requestTimeoutMs: 120_000,
});

export const NATIVE_ERRORS = [
  'unsupported-version',
  'invalid-message',
  'busy',
  'not-enrolled',
  'issuer-mismatch',
  'consent-denied',
  'grant-invalid',
  'binding-mismatch',
  'transport-mismatch',
  'capture-unavailable',
  'cancelled',
  'stale-operation',
  'timeout',
  'stopped',
  'internal',
] as const;
export type NativeErrorCode = (typeof NATIVE_ERRORS)[number];

export class NativeBridgeError extends Error {
  constructor(readonly code: NativeErrorCode) {
    super(code);
    this.name = 'NativeBridgeError';
  }
}

const ID = /^[A-Za-z0-9_-]{8,128}$/;
const REQUEST_ID = /^[A-Za-z0-9_-]{8,64}$/;
const B64_32 = /^[A-Za-z0-9_-]{43}$/;
const SIGNATURE = /^[A-Za-z0-9_-]{86}$/;
const HEX_SHA256 = /^[0-9a-f]{64}$/;
const TRACK = /^TR_[A-Za-z0-9]{4,64}$/;
const ROOM = /^![A-Za-z0-9._~\-+/=]{1,255}(:[A-Za-z0-9.\-:[\]]+)?$/;
const MXID = /^@[^\s:]{1,255}:[^\s]{1,255}$/;
const DEVICE = /^[A-Za-z0-9_-]{1,64}$/;
const ORIGIN = /^https:\/\/[a-z0-9.-]{1,253}(:\d{1,5})?$/;
const MID = /^[A-Za-z0-9_-]{1,32}$/;

type Rec = Record<string, unknown>;
function plain(value: unknown): value is Rec {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const proto = Object.getPrototypeOf(value) as unknown;
  return proto === Object.prototype || proto === null;
}
function exact(r: Rec, keys: readonly string[]): boolean {
  const own = Object.keys(r).sort();
  return own.length === keys.length && [...keys].sort().every((k, i) => own[i] === k);
}
const str = (v: unknown, re: RegExp): v is string => typeof v === 'string' && re.test(v);
const bounded = (v: unknown, max: number): v is string =>
  typeof v === 'string' && v.length > 0 && v.length <= max;

// Requests (page → endpoint) -------------------------------------------------------------------

/**
 * Each operation's fields (besides the envelope `p`, `v`, `id`, `op`) and their checks. The list
 * is closed: an unknown operation is `invalid-message`.
 */
const REQUESTS = {
  /** Negotiates the version; the answer describes, it never authorizes. */
  hello: {
    versions: (v: unknown) =>
      Array.isArray(v) &&
      v.length >= 1 &&
      v.length <= 4 &&
      v.every((x) => Number.isSafeInteger(x) && (x as number) >= 1),
  },
  /** The endpoint's public key and the issuer fingerprint it pinned for this origin. */
  'enroll.key': { origin: (v: unknown) => str(v, ORIGIN) },
  /** Signs the enrollment proof statement after its own local enrollment confirmation. */
  'enroll.prove': {
    endpointId: (v: unknown) => str(v, ID),
    challenge: (v: unknown) => str(v, B64_32),
    account: (v: unknown) => str(v, MXID),
    device: (v: unknown) => str(v, DEVICE),
  },
  /** A native grant to verify and, after local consent, acknowledge. */
  'grant.offer': {
    session: (v: unknown) => str(v, ID),
    grant: (v: unknown) => bounded(v, NATIVE_LIMITS.grantChars),
  },
  /** The helper's offer for the native input peer, relayed over Olm by this page. */
  'peer.offer': {
    session: (v: unknown) => str(v, ID),
    grantId: (v: unknown) => str(v, ID),
    sdp: (v: unknown) => bounded(v, NATIVE_LIMITS.sdpChars),
  },
  'peer.candidate': {
    session: (v: unknown) => str(v, ID),
    candidate: (v: unknown) => typeof v === 'string' && v.length <= NATIVE_LIMITS.candidateChars,
    mid: (v: unknown) => v === null || str(v, MID),
  },
  'peer.close': { session: (v: unknown) => str(v, ID) },
  /** Opens the endpoint's own source picker for a new capture operation. */
  'capture.start': { operation: (v: unknown) => str(v, ID), room: (v: unknown) => str(v, ROOM) },
  /** Records the media server's track SID after the page published the captured track. */
  'capture.bind': {
    operation: (v: unknown) => str(v, ID),
    trackSid: (v: unknown) => str(v, TRACK),
  },
  'capture.cancel': { operation: (v: unknown) => str(v, ID) },
  'capture.stop': { operation: (v: unknown) => str(v, ID) },
  status: {},
  /** Stops all native Assist work at once (also available locally without the page). */
  stop: {},
} as const satisfies Record<string, Record<string, (v: unknown) => boolean>>;

export type NativeOp = keyof typeof REQUESTS;
export const NATIVE_OPS = Object.keys(REQUESTS) as NativeOp[];

export interface NativeRequest {
  p: typeof NATIVE_PROTOCOL;
  v: 1;
  id: string;
  op: NativeOp;
  [field: string]: unknown;
}

function parseJson(raw: unknown): unknown {
  if (typeof raw !== 'string') return raw;
  if (raw.length > NATIVE_LIMITS.messageChars) throw new NativeBridgeError('invalid-message');
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    throw new NativeBridgeError('invalid-message');
  }
}

function envelope(m: Rec): void {
  if (m.p !== NATIVE_PROTOCOL) throw new NativeBridgeError('invalid-message');
  if (m.v !== NATIVE_VERSION) throw new NativeBridgeError('unsupported-version');
}

export function parseRequest(raw: unknown): NativeRequest {
  const m = parseJson(raw);
  if (!plain(m)) throw new NativeBridgeError('invalid-message');
  envelope(m);
  if (!str(m.id, REQUEST_ID) || typeof m.op !== 'string' || !Object.hasOwn(REQUESTS, m.op)) {
    throw new NativeBridgeError('invalid-message');
  }
  const fields = REQUESTS[m.op as NativeOp] as Record<string, (v: unknown) => boolean>;
  if (!exact(m, ['p', 'v', 'id', 'op', ...Object.keys(fields)])) {
    throw new NativeBridgeError('invalid-message');
  }
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m[k])) throw new NativeBridgeError('invalid-message');
  }
  return m as NativeRequest;
}

// Results (endpoint → page) --------------------------------------------------------------------

const RESULTS: Record<NativeOp, Record<string, (v: unknown) => boolean>> = {
  hello: {
    version: (v) => v === NATIVE_VERSION,
    /** Descriptive only (what is installed); never used to decide access. */
    features: (v) =>
      plain(v) &&
      exact(v, ['capture', 'clipboard', 'files', 'input']) &&
      Object.values(v).every((x) => typeof x === 'boolean'),
  },
  'enroll.key': {
    publicKey: (v) => str(v, B64_32),
    issuerFingerprint: (v) => str(v, HEX_SHA256),
    /** `release`: pinned by the signed release; `confirmed`: confirmed locally out of band. */
    pin: (v) => v === 'release' || v === 'confirmed',
  },
  'enroll.prove': { signature: (v) => str(v, SIGNATURE) },
  'grant.offer': { signature: (v) => str(v, SIGNATURE) },
  'peer.offer': {
    sdp: (v) => bounded(v, NATIVE_LIMITS.sdpChars),
    /** Signature over the transport statement (docs/49 §4) by the endpoint key. */
    transportSignature: (v) => str(v, SIGNATURE),
  },
  'peer.candidate': {},
  'peer.close': {},
  'capture.start': { operation: (v) => str(v, ID) },
  'capture.bind': {},
  'capture.cancel': {},
  'capture.stop': {},
  status: {
    endpoint: (v) =>
      plain(v) &&
      exact(v, ['endpointId', 'enrolled', 'pin']) &&
      typeof v.enrolled === 'boolean' &&
      (v.endpointId === null || str(v.endpointId, ID)) &&
      (v.pin === 'none' || v.pin === 'release' || v.pin === 'confirmed' || v.pin === 'mismatch'),
    capture: (v) =>
      plain(v) &&
      exact(v, ['operation', 'state', 'trackSid']) &&
      (v.operation === null || str(v.operation, ID)) &&
      CAPTURE_STATES.includes(v.state as CaptureState) &&
      (v.trackSid === null || str(v.trackSid, TRACK)),
    controller: (v) =>
      plain(v) &&
      exact(v, ['session', 'state']) &&
      (v.session === null || str(v.session, ID)) &&
      CONTROLLER_STATES.includes(v.state as ControllerState),
  },
  stop: {},
};

export const CAPTURE_STATES = [
  'idle',
  'picking',
  'live',
  'stopped',
  'cancelled',
  'failed',
] as const;
export type CaptureState = (typeof CAPTURE_STATES)[number];
export const CONTROLLER_STATES = ['idle', 'consent', 'active', 'suspended'] as const;
export type ControllerState = (typeof CONTROLLER_STATES)[number];

export type NativeResponse =
  | { p: typeof NATIVE_PROTOCOL; v: 1; id: string; ok: true; result: Rec }
  | { p: typeof NATIVE_PROTOCOL; v: 1; id: string; ok: false; error: NativeErrorCode };

/** Parses the endpoint's answer to the request `op` (the page knows which op it asked). */
export function parseResponse(raw: unknown, op: NativeOp): NativeResponse {
  const m = parseJson(raw);
  if (!plain(m)) throw new NativeBridgeError('invalid-message');
  envelope(m);
  if (!str(m.id, REQUEST_ID)) throw new NativeBridgeError('invalid-message');
  if (m.ok === false) {
    if (!exact(m, ['p', 'v', 'id', 'ok', 'error'])) throw new NativeBridgeError('invalid-message');
    if (!NATIVE_ERRORS.includes(m.error as NativeErrorCode)) {
      throw new NativeBridgeError('invalid-message');
    }
    return m as NativeResponse;
  }
  if (m.ok !== true || !exact(m, ['p', 'v', 'id', 'ok', 'result']) || !plain(m.result)) {
    throw new NativeBridgeError('invalid-message');
  }
  const fields = RESULTS[op];
  if (!exact(m.result, Object.keys(fields))) throw new NativeBridgeError('invalid-message');
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m.result[k])) throw new NativeBridgeError('invalid-message');
  }
  return m as NativeResponse;
}

// Events (endpoint → page) ---------------------------------------------------------------------

const EVENTS = {
  /** A candidate for the native input peer, to relay to the helper over Olm. */
  'peer.candidate': {
    session: (v: unknown) => str(v, ID),
    candidate: (v: unknown) => typeof v === 'string' && v.length <= NATIVE_LIMITS.candidateChars,
    mid: (v: unknown) => v === null || str(v, MID),
  },
  'capture.status': {
    operation: (v: unknown) => str(v, ID),
    state: (v: unknown) => CAPTURE_STATES.includes(v as CaptureState),
    reason: (v: unknown) =>
      v === null ||
      (typeof v === 'string' &&
        [
          'picker-cancelled',
          'source-lost',
          'track-ended',
          'local-stop',
          'reload',
          'replaced',
          'denied',
          'error',
        ].includes(v)),
  },
  /** Native controller lifecycle: suspension and stop are decided locally and are terminal for input. */
  lifecycle: {
    session: (v: unknown) => v === null || str(v, ID),
    state: (v: unknown) => CONTROLLER_STATES.includes(v as ControllerState) || v === 'stopped',
    reason: (v: unknown) =>
      v === null ||
      (typeof v === 'string' &&
        [
          'local-stop',
          'lease-lost',
          'revoked',
          'expired',
          'lock',
          'sleep',
          'desktop-change',
          'source-change',
          'replaced',
          'error',
        ].includes(v)),
  },
} as const satisfies Record<string, Record<string, (v: unknown) => boolean>>;

export type NativeEventName = keyof typeof EVENTS;

export interface NativeEvent {
  p: typeof NATIVE_PROTOCOL;
  v: 1;
  event: NativeEventName;
  /** Strictly increasing per bridge instance; a lower or equal value is dropped. */
  seq: number;
  [field: string]: unknown;
}

export function parseEvent(raw: unknown): NativeEvent {
  const m = parseJson(raw);
  if (!plain(m)) throw new NativeBridgeError('invalid-message');
  envelope(m);
  if (typeof m.event !== 'string' || !Object.hasOwn(EVENTS, m.event)) {
    throw new NativeBridgeError('invalid-message');
  }
  if (typeof m.seq !== 'number' || !Number.isSafeInteger(m.seq) || m.seq < 1) {
    throw new NativeBridgeError('invalid-message');
  }
  const fields = EVENTS[m.event as NativeEventName] as Record<string, (v: unknown) => boolean>;
  if (!exact(m, ['p', 'v', 'event', 'seq', ...Object.keys(fields)])) {
    throw new NativeBridgeError('invalid-message');
  }
  for (const [k, check] of Object.entries(fields)) {
    if (!check(m[k])) throw new NativeBridgeError('invalid-message');
  }
  return m as NativeEvent;
}

// Signed statements (endpoint key) ---------------------------------------------------------------

const enc = new TextEncoder();

/** Native input peer: the endpoint vouches for its own DTLS certificate, bound to the helper's. */
export function endpointTransportStatement(
  session: string,
  endpointId: string,
  endpointFingerprint: string,
  helperTransport: string,
): Uint8Array {
  return enc.encode(
    `palisade.assist.control/endpoint-transport/v2\n${session}\n${endpointId}\n${endpointFingerprint}\n${helperTransport}`,
  );
}

/** The attribute on the endpoint-owned `<video>` element that carries a capture operation. */
export const CAPTURE_ELEMENT_ATTRIBUTE = 'data-palisade-assist-capture';
