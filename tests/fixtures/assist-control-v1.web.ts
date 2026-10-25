/**
 * Palisade Assist control contract, `palisade.assist.control` version 1
 * (docs/47-ASSIST-CONTROL-PROTOCOL.md). Separate from the description-only desktop bridge
 * (`palisade.assist.bridge` v1, docs/45), which stays unchanged.
 *
 * Authority comes only from service-issued, Ed25519-signed grants that bind one Assist session,
 * both accounts and Matrix devices, the guest's WebRTC certificate fingerprint and (for native
 * input) an enrolled native endpoint. Nothing in a message, a bridge report or a media token is
 * authority. Every parser here is strict: exact keys, types, ranges and sizes, or a typed error.
 */

export const CONTROL_PROTOCOL = 'palisade.assist.control';
export const CONTROL_VERSION = 1;

/** Each grantable capability. Viewing is the Assist session itself and needs no grant. */
export const CAPABILITIES = [
  'pointer',
  'annotate',
  'control_mouse',
  'control_keyboard',
  'clipboard_to_host',
  'clipboard_to_guest',
  'file_to_host',
  'file_to_guest',
] as const;
export type Capability = (typeof CAPABILITIES)[number];

/** Capabilities executed by the host's operating system: they need an acknowledged endpoint. */
export const NATIVE_CAPABILITIES: readonly Capability[] = ['control_mouse', 'control_keyboard'];

export const LIMITS = Object.freeze({
  /** Largest JSON message on any channel (characters). */
  messageChars: 16_384,
  /** Largest clipboard text (UTF-8 bytes). */
  clipboardBytes: 65_536,
  /** Largest single file. */
  fileBytes: 1024 ** 3,
  /** File data frame payload. */
  chunkBytes: 65_536,
  /** Concurrent transfers per direction. */
  concurrentTransfers: 1,
  /** Control messages per second per sender (token bucket burst = 2 x). */
  controlPerSecond: 120,
  /** Clipboard and file control messages per second per sender. */
  dataPerSecond: 20,
  filenameChars: 180,
  heldKeys: 16,
});

export type ErrorCode =
  | 'incompatible-protocol'
  | 'invalid-message'
  | 'too-large'
  | 'binding-mismatch'
  | 'stale-generation'
  | 'replay'
  | 'not-granted'
  | 'expired'
  | 'revoked'
  | 'endpoint-unavailable'
  | 'consent-required'
  | 'rate-limited'
  | 'transfer-cancelled'
  | 'integrity-failure';

export class ControlError extends Error {
  constructor(readonly code: ErrorCode) {
    super(code);
    this.name = 'ControlError';
  }
}

/** The service-issued grant (signed fields, exactly these keys). */
export interface Grant {
  issuer: 'palisade';
  protocol: typeof CONTROL_PROTOCOL;
  version: 1;
  grantId: string;
  session: string;
  capability: Capability;
  generation: number;
  nonce: string;
  issuedAt: number;
  /** Never later than the Assist session's absolute deadline. */
  expiresAt: number;
  hostAccount: string;
  hostDevice: string;
  guestAccount: string;
  guestDevice: string;
  /** SHA-256 fingerprint of the guest's WebRTC certificate (`sha-256 AA:BB:…`). */
  guestTransport: string;
  /** Enrolled native endpoint, for native capabilities; otherwise null. */
  endpoint: string | null;
}

export interface SignedGrant {
  grant: Grant;
  /** Base64url Ed25519 signature over `canonicalGrant(grant)` by the service issuer key. */
  signature: string;
}

const GRANT_KEYS = [
  'capability',
  'endpoint',
  'expiresAt',
  'generation',
  'grantId',
  'guestAccount',
  'guestDevice',
  'guestTransport',
  'hostAccount',
  'hostDevice',
  'issuedAt',
  'issuer',
  'nonce',
  'protocol',
  'session',
  'version',
] as const;

const ID = /^[A-Za-z0-9_-]{8,128}$/;
const NONCE = /^[A-Za-z0-9_-]{16,128}$/;
const MXID = /^@[^\s:]{1,255}:[^\s]{1,255}$/;
const DEVICE = /^[A-Za-z0-9_-]{1,64}$/;
const FINGERPRINT = /^sha-256 (?:[0-9A-F]{2}:){31}[0-9A-F]{2}$/;
const SIGNATURE = /^[A-Za-z0-9_-]{86}$/;

function plain(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const proto = Object.getPrototypeOf(value) as unknown;
  return proto === Object.prototype || proto === null;
}

function exactKeys(record: Record<string, unknown>, keys: readonly string[]): boolean {
  const own = Object.keys(record).sort();
  return own.length === keys.length && [...keys].sort().every((key, i) => own[i] === key);
}

function int(value: unknown, min: number, max: number): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= min && value <= max;
}

export function isCapability(value: unknown): value is Capability {
  return typeof value === 'string' && (CAPABILITIES as readonly string[]).includes(value);
}

/** Validates the shape of a grant (not its signature: see `verifyGrant`). */
export function parseGrant(value: unknown): Grant {
  if (!plain(value) || !exactKeys(value, GRANT_KEYS)) throw new ControlError('invalid-message');
  const g = value;
  if (g.issuer !== 'palisade' || g.protocol !== CONTROL_PROTOCOL) {
    throw new ControlError('incompatible-protocol');
  }
  if (g.version !== CONTROL_VERSION) throw new ControlError('incompatible-protocol');
  const ok =
    typeof g.grantId === 'string' &&
    ID.test(g.grantId) &&
    typeof g.session === 'string' &&
    ID.test(g.session) &&
    isCapability(g.capability) &&
    int(g.generation, 1, 1_000_000) &&
    typeof g.nonce === 'string' &&
    NONCE.test(g.nonce) &&
    int(g.issuedAt, 0, Number.MAX_SAFE_INTEGER) &&
    int(g.expiresAt, 0, Number.MAX_SAFE_INTEGER) &&
    g.expiresAt > g.issuedAt &&
    g.expiresAt - g.issuedAt <= 15 * 60_000 &&
    typeof g.hostAccount === 'string' &&
    MXID.test(g.hostAccount) &&
    typeof g.guestAccount === 'string' &&
    MXID.test(g.guestAccount) &&
    g.hostAccount !== g.guestAccount &&
    typeof g.hostDevice === 'string' &&
    DEVICE.test(g.hostDevice) &&
    typeof g.guestDevice === 'string' &&
    DEVICE.test(g.guestDevice) &&
    typeof g.guestTransport === 'string' &&
    FINGERPRINT.test(g.guestTransport) &&
    (g.endpoint === null || (typeof g.endpoint === 'string' && ID.test(g.endpoint)));
  if (!ok) throw new ControlError('invalid-message');
  const native = NATIVE_CAPABILITIES.includes(g.capability as Capability);
  if (native !== (g.endpoint !== null)) throw new ControlError('invalid-message');
  return g as unknown as Grant;
}

/** Deterministic signed bytes: the grant's keys in sorted order, compact JSON, UTF-8. */
export function canonicalGrant(grant: Grant): Uint8Array {
  const ordered: Record<string, unknown> = {};
  for (const key of GRANT_KEYS) ordered[key] = grant[key];
  return new TextEncoder().encode(JSON.stringify(ordered));
}

export function base64urlToBytes(text: string): Uint8Array {
  const base64 = text.replace(/-/g, '+').replace(/_/g, '/');
  const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
}

/**
 * Verifies a signed grant against a trusted issuer public key (raw 32-byte Ed25519) and the
 * receiver's own expectations. Returns the grant or throws a typed error.
 */
export async function verifyGrant(
  signed: unknown,
  issuerKey: Uint8Array,
  expect: {
    session: string;
    hostAccount: string;
    hostDevice: string;
    guestAccount: string;
    guestDevice: string;
    now: number;
    /** The absolute Assist deadline from the session, which no grant may exceed. */
    sessionDeadline: number;
    guestTransport?: string;
    minimumGeneration?: number;
  },
): Promise<Grant> {
  if (!plain(signed) || !exactKeys(signed, ['grant', 'signature'])) {
    throw new ControlError('invalid-message');
  }
  if (typeof signed.signature !== 'string' || !SIGNATURE.test(signed.signature)) {
    throw new ControlError('invalid-message');
  }
  const grant = parseGrant(signed.grant);
  const key = await crypto.subtle.importKey(
    'raw',
    issuerKey.slice().buffer,
    { name: 'Ed25519' },
    false,
    ['verify'],
  );
  const valid = await crypto.subtle.verify(
    { name: 'Ed25519' },
    key,
    base64urlToBytes(signed.signature).slice().buffer,
    canonicalGrant(grant).slice().buffer,
  );
  if (!valid) throw new ControlError('binding-mismatch');
  const bound =
    grant.session === expect.session &&
    grant.hostAccount === expect.hostAccount &&
    grant.hostDevice === expect.hostDevice &&
    grant.guestAccount === expect.guestAccount &&
    grant.guestDevice === expect.guestDevice &&
    (expect.guestTransport === undefined || grant.guestTransport === expect.guestTransport);
  if (!bound) throw new ControlError('binding-mismatch');
  if (grant.expiresAt > expect.sessionDeadline) throw new ControlError('binding-mismatch');
  if (grant.expiresAt <= expect.now || grant.issuedAt > expect.now + 30_000) {
    throw new ControlError('expired');
  }
  if (expect.minimumGeneration !== undefined && grant.generation < expect.minimumGeneration) {
    throw new ControlError('stale-generation');
  }
  return grant;
}

// Data-channel messages ---------------------------------------------------------------------

/** Physical keys a guest may send (no OS/meta keys; Alt+Tab and Ctrl+Alt+Del are refused). */
export const KEY_CODES: readonly string[] = [
  ...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('').map((c) => `Key${c}`),
  ...'0123456789'.split('').map((d) => `Digit${d}`),
  ...Array.from({ length: 12 }, (_, i) => `F${String(i + 1)}`),
  'ControlLeft',
  'ControlRight',
  'ShiftLeft',
  'ShiftRight',
  'AltLeft',
  'AltRight',
  'Enter',
  'Escape',
  'Tab',
  'Space',
  'Backspace',
  'Delete',
  'Insert',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Minus',
  'Equal',
  'BracketLeft',
  'BracketRight',
  'Backslash',
  'Semicolon',
  'Quote',
  'Backquote',
  'Comma',
  'Period',
  'Slash',
];

/** Named shortcuts the guest's virtual keyboard may send. */
export const SHORTCUTS = Object.freeze({
  copy: ['ControlLeft', 'KeyC'],
  paste: ['ControlLeft', 'KeyV'],
  cut: ['ControlLeft', 'KeyX'],
  selectAll: ['ControlLeft', 'KeyA'],
  undo: ['ControlLeft', 'KeyZ'],
  redo: ['ControlLeft', 'KeyY'],
  save: ['ControlLeft', 'KeyS'],
  find: ['ControlLeft', 'KeyF'],
  nextTab: ['ControlLeft', 'Tab'],
  previousTab: ['ControlLeft', 'ShiftLeft', 'Tab'],
} as const);
export type ShortcutName = keyof typeof SHORTCUTS;

interface Envelope {
  p: typeof CONTROL_PROTOCOL;
  v: 1;
  session: string;
  grant: string;
  gen: number;
  seq: number;
}

export type ControlMessage = Envelope &
  (
    | { kind: 'pointer-move'; x: number; y: number; sourceGen: number }
    | { kind: 'button'; button: 'left' | 'middle' | 'right'; down: boolean; sourceGen: number }
    | { kind: 'wheel'; dx: number; dy: number; sourceGen: number }
    | { kind: 'key'; code: string; down: boolean }
    | { kind: 'shortcut'; name: ShortcutName }
    | { kind: 'release-all' }
    | { kind: 'clipboard-text'; text: string }
    | { kind: 'file-offer'; transfer: string; name: string; size: number; sha256: string }
    | { kind: 'file-accept'; transfer: string }
    | { kind: 'file-reject'; transfer: string }
    | { kind: 'file-cancel'; transfer: string }
    | { kind: 'file-complete'; transfer: string; chunks: number }
  );
export type ControlKind = ControlMessage['kind'];

const PAYLOAD_KEYS: Readonly<Record<ControlKind, readonly string[]>> = {
  'pointer-move': ['x', 'y', 'sourceGen'],
  button: ['button', 'down', 'sourceGen'],
  wheel: ['dx', 'dy', 'sourceGen'],
  key: ['code', 'down'],
  shortcut: ['name'],
  'release-all': [],
  'clipboard-text': ['text'],
  'file-offer': ['transfer', 'name', 'size', 'sha256'],
  'file-accept': ['transfer'],
  'file-reject': ['transfer'],
  'file-cancel': ['transfer'],
  'file-complete': ['transfer', 'chunks'],
};

/** Which capability authorizes each message kind (sender's perspective). */
export function capabilityFor(
  kind: ControlKind,
  from: 'host' | 'guest',
): Capability | 'transfer-reply' | undefined {
  switch (kind) {
    case 'pointer-move':
    case 'button':
    case 'wheel':
      return from === 'guest' ? 'control_mouse' : undefined;
    case 'key':
    case 'shortcut':
      return from === 'guest' ? 'control_keyboard' : undefined;
    case 'release-all':
      return from === 'guest' ? 'control_keyboard' : undefined;
    case 'clipboard-text':
      return from === 'guest' ? 'clipboard_to_host' : 'clipboard_to_guest';
    case 'file-offer':
    case 'file-complete':
      return from === 'guest' ? 'file_to_host' : 'file_to_guest';
    case 'file-accept':
    case 'file-reject':
    case 'file-cancel':
      return 'transfer-reply';
  }
}

const TRANSFER = /^[A-Za-z0-9_-]{16,64}$/;
const SHA256 = /^[0-9a-f]{64}$/;
// Windows-reserved device names, control/format characters, separators and trailing dots/spaces.
const BAD_NAME =
  // eslint-disable-next-line no-control-regex -- filenames must reject control characters
  /[\u0000-\u001f\u007f<>:"/\\|?*\u202a-\u202e\u2066-\u2069]|[. ]$|^\.+$|^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\.|$)/i;

export function safeFilename(name: unknown): name is string {
  return (
    typeof name === 'string' &&
    name.length >= 1 &&
    name.length <= LIMITS.filenameChars &&
    name === name.trim() &&
    name === name.normalize('NFC') &&
    !BAD_NAME.test(name)
  );
}

/** Parses one JSON data-channel message. Never evaluates or follows anything in it. */
export function parseControlMessage(raw: unknown): ControlMessage {
  let value: unknown = raw;
  if (typeof raw === 'string') {
    if (raw.length > LIMITS.messageChars) throw new ControlError('too-large');
    try {
      value = JSON.parse(raw) as unknown;
    } catch {
      throw new ControlError('invalid-message');
    }
  }
  if (!plain(value)) throw new ControlError('invalid-message');
  const m = value;
  if (m.p !== CONTROL_PROTOCOL || m.v !== CONTROL_VERSION) {
    throw new ControlError('incompatible-protocol');
  }
  const kind = m.kind;
  if (typeof kind !== 'string' || !Object.hasOwn(PAYLOAD_KEYS, kind)) {
    throw new ControlError('invalid-message');
  }
  const keys = [
    'p',
    'v',
    'session',
    'grant',
    'gen',
    'seq',
    'kind',
    ...PAYLOAD_KEYS[kind as ControlKind],
  ];
  if (!exactKeys(m, keys)) throw new ControlError('invalid-message');
  const envelopeOk =
    typeof m.session === 'string' &&
    ID.test(m.session) &&
    typeof m.grant === 'string' &&
    ID.test(m.grant) &&
    int(m.gen, 1, 1_000_000) &&
    int(m.seq, 0, Number.MAX_SAFE_INTEGER);
  if (!envelopeOk) throw new ControlError('invalid-message');
  const unit = (n: unknown) => typeof n === 'number' && Number.isFinite(n) && n >= 0 && n <= 1;
  const ok = ((): boolean => {
    switch (kind as ControlKind) {
      case 'pointer-move':
        return unit(m.x) && unit(m.y) && int(m.sourceGen, 1, 1_000_000);
      case 'button':
        return (
          ['left', 'middle', 'right'].includes(m.button as string) &&
          typeof m.down === 'boolean' &&
          int(m.sourceGen, 1, 1_000_000)
        );
      case 'wheel':
        return int(m.dx, -1200, 1200) && int(m.dy, -1200, 1200) && int(m.sourceGen, 1, 1_000_000);
      case 'key':
        return (
          typeof m.code === 'string' && KEY_CODES.includes(m.code) && typeof m.down === 'boolean'
        );
      case 'shortcut':
        return typeof m.name === 'string' && Object.hasOwn(SHORTCUTS, m.name);
      case 'release-all':
        return true;
      case 'clipboard-text':
        return (
          typeof m.text === 'string' &&
          m.text.length > 0 &&
          !m.text.includes('\u0000') &&
          new TextEncoder().encode(m.text).byteLength <= LIMITS.clipboardBytes
        );
      case 'file-offer':
        return (
          typeof m.transfer === 'string' &&
          TRANSFER.test(m.transfer) &&
          safeFilename(m.name) &&
          int(m.size, 0, LIMITS.fileBytes) &&
          typeof m.sha256 === 'string' &&
          SHA256.test(m.sha256)
        );
      case 'file-accept':
      case 'file-reject':
      case 'file-cancel':
        return typeof m.transfer === 'string' && TRANSFER.test(m.transfer);
      case 'file-complete':
        return (
          typeof m.transfer === 'string' &&
          TRANSFER.test(m.transfer) &&
          int(m.chunks, 0, Math.ceil(LIMITS.fileBytes / LIMITS.chunkBytes))
        );
    }
  })();
  if (!ok) throw new ControlError(kind === 'clipboard-text' ? 'too-large' : 'invalid-message');
  return m as unknown as ControlMessage;
}

/**
 * Per-peer receive state: strictly increasing sequence numbers per grant and generation, so a
 * replayed or reordered message is refused, and a newer generation invalidates older traffic.
 */
export function createReplayGuard() {
  const last = new Map<string, { gen: number; seq: number }>();
  return (message: { grant: string; gen: number; seq: number }): void => {
    const seen = last.get(message.grant);
    if (seen && message.gen < seen.gen) throw new ControlError('stale-generation');
    if (seen && message.gen === seen.gen && message.seq <= seen.seq) {
      throw new ControlError('replay');
    }
    last.set(message.grant, { gen: message.gen, seq: message.seq });
  };
}

/** Token bucket: `perSecond` sustained, bursts of twice that. */
export function createRateLimit(perSecond: number) {
  let tokens = perSecond * 2;
  let at = 0;
  return (now: number): void => {
    tokens = Math.min(perSecond * 2, tokens + ((now - at) / 1000) * perSecond);
    at = now;
    if (tokens < 1) throw new ControlError('rate-limited');
    tokens -= 1;
  };
}

// Binary file frames: [4-byte big-endian chunk index][payload ≤ 64 KiB], one transfer at a time.
export function encodeChunk(index: number, payload: Uint8Array): ArrayBuffer {
  if (!int(index, 0, 0xffffffff) || payload.byteLength > LIMITS.chunkBytes) {
    throw new ControlError('too-large');
  }
  const frame = new Uint8Array(4 + payload.byteLength);
  new DataView(frame.buffer).setUint32(0, index);
  frame.set(payload, 4);
  return frame.buffer;
}

export function decodeChunk(frame: ArrayBuffer): { index: number; payload: Uint8Array } {
  if (frame.byteLength < 4 || frame.byteLength > 4 + LIMITS.chunkBytes) {
    throw new ControlError('too-large');
  }
  return {
    index: new DataView(frame).getUint32(0),
    payload: new Uint8Array(frame, 4),
  };
}

export async function sha256Hex(data: ArrayBuffer | Uint8Array): Promise<string> {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  const digest = await crypto.subtle.digest('SHA-256', bytes.slice().buffer);
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}
