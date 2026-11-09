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
  /** Profile 2 (docs/51): largest clipboard image (encoded PNG bytes). */
  imageBytes: 16 * 1024 ** 2,
  /** Profile 2: largest clipboard image side and pixel count. */
  imageSide: 16_384,
  imagePixels: 33_554_432,
  /** Profile 2: largest `input-text` (UTF-16 code units). */
  textInputChars: 256,
  /**
   * Profile 2: an input event older than this (relative to the fastest event seen on the same
   * peer) is dropped by the receiver; releases are always applied (docs/51 §4.4).
   */
  inputMaxAgeMs: 300,
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
  | 'integrity-failure'
  /** Profile 2: the receiver did not declare the feature this kind needs. */
  | 'unsupported'
  /** Profile 2: an input event older than `inputMaxAgeMs`. */
  | 'stale-input'
  /** Profile 2: `input-text` while modifier keys are held. */
  | 'held-input'
  /** Cooperative input (docs/51 §4.6): the local person has priority; remote input is paused. */
  | 'yielded'
  /** Cooperative input: a barrier or status for another epoch, or input sent before the barrier. */
  | 'stale-epoch';

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

// Profile 2 (docs/51-ASSIST-FEATURE-COMPLETION.md) -------------------------------------------

/**
 * Data-plane features a receiver may declare (an endpoint at enrollment, a Palisade page over
 * Olm). A sender uses a profile 2 kind only when the receiving side declared its feature;
 * declarations are descriptive and never authority. Version 1 sessions refuse every profile 2 kind.
 */
export const PROFILE2_FEATURES = [
  'input2',
  'clipboardImage',
  'clipboardSync',
  'fileResume',
] as const;
export type Profile2Feature = (typeof PROFILE2_FEATURES)[number];

/**
 * Physical keys for `input-key` (profile 2): every version 1 key plus function keys to F24, the
 * numeric keypad, lock keys, the context-menu key and international/IME keys. Still no OS/meta
 * keys, no Print Screen, Pause or power/media keys. Codes are W3C `KeyboardEvent.code` values;
 * the receiving computer's own keyboard layout interprets them (physical mode, docs/51 §4.2).
 */
export const KEY_CODES_2: readonly string[] = [
  ...KEY_CODES,
  ...Array.from({ length: 12 }, (_, i) => `F${String(i + 13)}`),
  ...'0123456789'.split('').map((d) => `Numpad${d}`),
  'NumpadAdd',
  'NumpadSubtract',
  'NumpadMultiply',
  'NumpadDivide',
  'NumpadDecimal',
  'NumpadEnter',
  'NumLock',
  'CapsLock',
  'ScrollLock',
  'ContextMenu',
  'IntlBackslash',
  'IntlRo',
  'IntlYen',
  'KanaMode',
  'Lang1',
  'Lang2',
  'Convert',
  'NonConvert',
];

/** Modifier keys: `input-text` is refused while any of them is held (docs/51 §4.3). */
export const MODIFIER_CODES: readonly string[] = [
  'ControlLeft',
  'ControlRight',
  'ShiftLeft',
  'ShiftRight',
  'AltLeft',
  'AltRight',
];

/** Mouse buttons for `input-button` (profile 2): version 1's three plus back and forward. */
export const BUTTONS_2 = ['left', 'middle', 'right', 'back', 'forward'] as const;
export type Button2 = (typeof BUTTONS_2)[number];

/** Clipboard image formats (profile 2). PNG only; anything else is `unsupported`. */
export const IMAGE_FORMATS = ['image/png'] as const;

/** Which declared feature each profile 2 kind needs from the receiver. */
export const PROFILE2_KIND_FEATURE = Object.freeze({
  'input-move': 'input2',
  'input-button': 'input2',
  'input-wheel': 'input2',
  'input-key': 'input2',
  'input-text': 'input2',
  'input-shortcut': 'input2',
  'clipboard-image': 'clipboardImage',
  'clipboard-sync': 'clipboardSync',
  'file-resume-offer': 'fileResume',
  'file-resume-accept': 'fileResume',
} as const satisfies Record<string, Profile2Feature>);

// Cooperative local input (docs/51 §4.6) ---------------------------------------------------------

/**
 * Cooperative local input, version 1: the host keeps their own mouse and keyboard during an
 * approved session and has priority. Ordinary local activity makes the endpoint **yield** remote
 * input (nonterminal, separate from `lifecycle suspended/stopped`); only the endpoint's own
 * observation decides yielding and readiness, and the helper resumes with fresh gestures after a
 * readiness barrier. Negotiated by its own feature, outside profile 2's list (pinned fixtures):
 * the endpoint declares `localYield` (enrollment and extension version 3); the helper opts in on
 * each peer with `input-barrier {epoch: 0}` before any input.
 */
export const COOPERATIVE_FEATURES = ['localYield'] as const;
export type CooperativeFeature = (typeof COOPERATIVE_FEATURES)[number];
/** Every data-plane feature a receiver may declare. */
export type DataFeature = Profile2Feature | CooperativeFeature;

export const COOPERATIVE_KIND_FEATURE = Object.freeze({
  'input-state': 'localYield',
  'input-barrier': 'localYield',
} as const satisfies Record<string, CooperativeFeature>);

/** Why remote input is yielded (display only for the helper; never authority). */
export const YIELD_REASONS = ['local-input', 'local-held', 'neutralizing'] as const;
export type YieldReason = (typeof YIELD_REASONS)[number];

export const LOCAL_YIELD = Object.freeze({
  /** Local inactivity required (after cleanup and neutrality) before readiness. */
  idleMs: 2_000,
  /** Largest epoch; the next yield after it ends input authority instead. */
  maxEpoch: 2_147_483_647,
});

/**
 * Bounded text entry (`input-text`): 1–256 UTF-16 code units, NFC, well-formed (no lone
 * surrogates), no control characters (C0, DEL, C1), no noncharacters U+FFFE/U+FFFF. Enter, Tab
 * and Backspace are keys, never text.
 */
export function validInputText(text: unknown): text is string {
  if (typeof text !== 'string' || text.length < 1 || text.length > LIMITS.textInputChars)
    return false;
  if (text !== text.normalize('NFC')) return false;
  // eslint-disable-next-line no-control-regex -- control characters are exactly what is refused
  if (/[\u0000-\u001f\u007f-\u009f\ufffe\uffff]/.test(text)) return false;
  for (let i = 0; i < text.length; i += 1) {
    const c = text.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      const next = text.charCodeAt(i + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      i += 1;
    } else if (c >= 0xdc00 && c <= 0xdfff) {
      return false;
    }
  }
  return true;
}

interface Envelope {
  p: typeof CONTROL_PROTOCOL;
  v: 1 | 2;
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
    /**
     * Stream sessions only (v2, docs/50 §3): the receiver's integrity receipt for one transfer,
     * after it checked size and SHA-256 (`verified`), committed it (`saved`) or refused it.
     */
    | {
        kind: 'file-receipt';
        transfer: string;
        result: 'verified' | 'saved' | 'integrity-failure' | 'rejected' | 'cancelled';
        sha256: string;
      }
    // Profile 2 (docs/51), version 2 only, each needing a declared receiver feature.
    | { kind: 'input-move'; x: number; y: number; sourceGen: number; t: number }
    | { kind: 'input-button'; button: Button2; down: boolean; sourceGen: number; t: number }
    | { kind: 'input-wheel'; dx: number; dy: number; sourceGen: number; t: number }
    | { kind: 'input-key'; code: string; down: boolean; repeat: boolean; t: number }
    | { kind: 'input-text'; text: string; t: number }
    | { kind: 'input-shortcut'; name: ShortcutName; t: number }
    | {
        kind: 'clipboard-image';
        transfer: string;
        format: (typeof IMAGE_FORMATS)[number];
        width: number;
        height: number;
        size: number;
        sha256: string;
      }
    | { kind: 'clipboard-sync'; state: 'on' | 'off' }
    | {
        kind: 'file-resume-offer';
        transfer: string;
        previous: string;
        name: string;
        size: number;
        sha256: string;
      }
    | { kind: 'file-resume-accept'; transfer: string; from: number }
    /** Endpoint → helper (docs/51 §4.6): remote input yielded to the local person, or ready. */
    | { kind: 'input-state'; state: 'yielded' | 'ready'; epoch: number; reason: YieldReason | null }
    /** Helper → endpoint: opt-in (`epoch` 0) or "seen ready `epoch`; only fresh input follows". */
    | { kind: 'input-barrier'; epoch: number; t: number }
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
  'file-receipt': ['transfer', 'result', 'sha256'],
  'input-move': ['x', 'y', 'sourceGen', 't'],
  'input-button': ['button', 'down', 'sourceGen', 't'],
  'input-wheel': ['dx', 'dy', 'sourceGen', 't'],
  'input-key': ['code', 'down', 'repeat', 't'],
  'input-text': ['text', 't'],
  'input-shortcut': ['name', 't'],
  'clipboard-image': ['transfer', 'format', 'width', 'height', 'size', 'sha256'],
  'clipboard-sync': ['state'],
  'file-resume-offer': ['transfer', 'previous', 'name', 'size', 'sha256'],
  'file-resume-accept': ['transfer', 'from'],
  'input-state': ['state', 'epoch', 'reason'],
  'input-barrier': ['epoch', 't'],
};

/** Kinds that exist only in stream sessions (version 2): the receipt and all of profile 2. */
const V2_ONLY: readonly ControlKind[] = [
  'file-receipt',
  ...(Object.keys(PROFILE2_KIND_FEATURE) as ControlKind[]),
  ...(Object.keys(COOPERATIVE_KIND_FEATURE) as ControlKind[]),
];

/** Input kinds (version 1, profile 2, cooperative input): control channel only. */
export const INPUT_KINDS: readonly ControlKind[] = [
  'pointer-move',
  'button',
  'wheel',
  'key',
  'shortcut',
  'release-all',
  'input-move',
  'input-button',
  'input-wheel',
  'input-key',
  'input-text',
  'input-shortcut',
  'input-state',
  'input-barrier',
];

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
      // Version 1 only; version 2 accepts either input grant, scoped to it (`authorizes`).
      return from === 'guest' ? 'control_keyboard' : undefined;
    case 'input-move':
    case 'input-button':
    case 'input-wheel':
      return from === 'guest' ? 'control_mouse' : undefined;
    case 'input-key':
    case 'input-text':
    case 'input-shortcut':
      return from === 'guest' ? 'control_keyboard' : undefined;
    case 'clipboard-text':
    case 'clipboard-image':
    case 'clipboard-sync':
      return from === 'guest' ? 'clipboard_to_host' : 'clipboard_to_guest';
    case 'file-offer':
    case 'file-complete':
    case 'file-resume-offer':
      return from === 'guest' ? 'file_to_host' : 'file_to_guest';
    case 'file-accept':
    case 'file-reject':
    case 'file-cancel':
    case 'file-receipt':
    case 'file-resume-accept':
      return 'transfer-reply';
    case 'input-state':
    case 'input-barrier':
      // Cooperative input rides on either input grant (`authorizes`).
      return undefined;
  }
}

/**
 * Release authority, version 2 (docs/51 §4.5). `release-all` rides on **either** active input
 * grant of the session — `control_mouse` or `control_keyboard` — and its **scope is that grant**:
 * under `control_mouse` it releases only the buttons this peer pressed in this session, under
 * `control_keyboard` only the keys. It never authorizes any other input, never releases input it
 * did not press (the local person's, another session's or another peer's), and never needs the
 * other capability. Version 1 (docs/47, pinned) keeps `control_keyboard` (see `capabilityFor`).
 * The endpoint's own release of everything at revocation, suspension or teardown is separate and
 * always happens.
 */
export const RELEASE_SCOPE = Object.freeze({
  control_mouse: 'buttons',
  control_keyboard: 'keys',
} as const);
export type ReleaseScope = (typeof RELEASE_SCOPE)[keyof typeof RELEASE_SCOPE];

/** Whether a message of `kind` from `from` may ride on a grant of `capability` (`version`). */
export function authorizes(
  kind: ControlKind,
  from: 'host' | 'guest',
  capability: Capability,
  version: 1 | 2,
): boolean {
  if ((kind === 'release-all' || kind === 'input-barrier') && version === 2) {
    return from === 'guest' && Object.hasOwn(RELEASE_SCOPE, capability);
  }
  if (kind === 'input-state' && version === 2) {
    // The endpoint's status to the helper, under one of the helper's input grants on this peer.
    return from === 'host' && Object.hasOwn(RELEASE_SCOPE, capability);
  }
  return capabilityFor(kind, from) === capability;
}

type InputMessage = Pick<Envelope, 'session'> & {
  kind: ControlKind;
  button?: unknown;
  code?: unknown;
  down?: unknown;
};

/**
 * Reference receiver bookkeeping for one session's input on one peer (version 2), shared with
 * native endpoints through `docs/assist-control/v2-release/fixtures.json`. Call `apply` with a
 * message that already parsed, verified its grant and generation and passed replay and age
 * checks, together with the capability and current state of the grant it carries. It returns the
 * releases the endpoint must inject now; presses are executed by the caller and recorded here.
 */
export function createOwnedInput(session: string) {
  const buttons = new Set<string>();
  const keys = new Set<string>();
  const none = (): { buttons: string[]; keys: string[] } => ({ buttons: [], keys: [] });
  return {
    apply(
      message: InputMessage,
      grant: { capability: Capability; active: boolean },
    ): { buttons: string[]; keys: string[] } {
      if (message.session !== session) throw new ControlError('binding-mismatch');
      if (!grant.active) throw new ControlError('revoked');
      if (!INPUT_KINDS.includes(message.kind)) throw new ControlError('invalid-message');
      if (!authorizes(message.kind, 'guest', grant.capability, 2)) {
        throw new ControlError('not-granted');
      }
      switch (message.kind) {
        case 'button':
        case 'input-button': {
          const button = String(message.button);
          if (message.down === true) {
            buttons.add(button);
            return none();
          }
          // An up for a button this peer never pressed is ignored, never injected.
          return buttons.delete(button) ? { buttons: [button], keys: [] } : none();
        }
        case 'key':
        case 'input-key': {
          const code = String(message.code);
          if (message.down === true) {
            keys.add(code);
            return none();
          }
          return keys.delete(code) ? { buttons: [], keys: [code] } : none();
        }
        case 'release-all': {
          const scope = RELEASE_SCOPE[grant.capability as keyof typeof RELEASE_SCOPE];
          const owned = scope === 'buttons' ? buttons : keys;
          const released = [...owned].sort();
          owned.clear();
          return scope === 'buttons'
            ? { buttons: released, keys: [] }
            : { buttons: [], keys: released };
        }
        default:
          return none();
      }
    },
    /**
     * Cooperative input (docs/51 §4.6): what this peer pressed is let go when input yields or
     * ends, **except** what the local person is holding too — releasing that would release their
     * hold. Those are handed over (forgotten without injecting anything); they become neutral when
     * the local person lets go. Returns both lists; everything owned is cleared.
     */
    yieldTo(local: { buttons: readonly string[]; keys: readonly string[] }): {
      released: { buttons: string[]; keys: string[] };
      handedOver: { buttons: string[]; keys: string[] };
    } {
      const split = (owned: Set<string>, held: readonly string[]) => {
        const all = [...owned].sort();
        owned.clear();
        return {
          released: all.filter((x) => !held.includes(x)),
          handedOver: all.filter((x) => held.includes(x)),
        };
      };
      const b = split(buttons, local.buttons);
      const k = split(keys, local.keys);
      return {
        released: { buttons: b.released, keys: k.released },
        handedOver: { buttons: b.handedOver, keys: k.handedOver },
      };
    },
    /** Endpoint teardown (revocation, suspension, peer loss, stop): release everything owned. */
    releaseEverything(): { buttons: string[]; keys: string[] } {
      const released = { buttons: [...buttons].sort(), keys: [...keys].sort() };
      buttons.clear();
      keys.clear();
      return released;
    },
    get held(): { buttons: string[]; keys: string[] } {
      return { buttons: [...buttons].sort(), keys: [...keys].sort() };
    },
  };
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
/**
 * Parses a data-channel message. Version 1 sessions accept only `v: 1`; stream sessions
 * (docs/48) pass `2` and accept only `v: 2`, with the same kinds and fields.
 */
export function parseControlMessage(
  raw: unknown,
  version: 1 | 2 = CONTROL_VERSION,
  /**
   * The profile 2 features this receiver declared (docs/51). When given, a profile 2 kind whose
   * feature is not listed is refused as `unsupported`. Omitted: every implemented kind parses.
   */
  features?: ReadonlySet<DataFeature>,
): ControlMessage {
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
  if (m.p !== CONTROL_PROTOCOL || m.v !== version) {
    throw new ControlError('incompatible-protocol');
  }
  const kind = m.kind;
  if (typeof kind !== 'string' || !Object.hasOwn(PAYLOAD_KEYS, kind)) {
    throw new ControlError('invalid-message');
  }
  if (version === 1 && V2_ONLY.includes(kind as ControlKind)) {
    throw new ControlError('invalid-message');
  }
  const feature = (
    { ...PROFILE2_KIND_FEATURE, ...COOPERATIVE_KIND_FEATURE } as Record<
      string,
      DataFeature | undefined
    >
  )[kind];
  if (feature && features && !features.has(feature)) throw new ControlError('unsupported');
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
  const time = (n: unknown) => int(n, 0, Number.MAX_SAFE_INTEGER);
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
      case 'file-receipt':
        return (
          typeof m.transfer === 'string' &&
          TRANSFER.test(m.transfer) &&
          ['verified', 'saved', 'integrity-failure', 'rejected', 'cancelled'].includes(
            m.result as string,
          ) &&
          typeof m.sha256 === 'string' &&
          /^[0-9a-f]{64}$/.test(m.sha256)
        );
      case 'input-move':
        return unit(m.x) && unit(m.y) && int(m.sourceGen, 1, 1_000_000) && time(m.t);
      case 'input-button':
        return (
          (BUTTONS_2 as readonly string[]).includes(m.button as string) &&
          typeof m.down === 'boolean' &&
          int(m.sourceGen, 1, 1_000_000) &&
          time(m.t)
        );
      case 'input-wheel':
        return (
          int(m.dx, -1200, 1200) &&
          int(m.dy, -1200, 1200) &&
          (m.dx !== 0 || m.dy !== 0) &&
          int(m.sourceGen, 1, 1_000_000) &&
          time(m.t)
        );
      case 'input-key':
        return (
          typeof m.code === 'string' &&
          KEY_CODES_2.includes(m.code) &&
          typeof m.down === 'boolean' &&
          typeof m.repeat === 'boolean' &&
          // A repeat is an auto-repeated key-down; a key-up never repeats.
          (m.down || !m.repeat) &&
          time(m.t)
        );
      case 'input-text':
        return validInputText(m.text) && time(m.t);
      case 'input-shortcut':
        return typeof m.name === 'string' && Object.hasOwn(SHORTCUTS, m.name) && time(m.t);
      case 'clipboard-image':
        return (
          typeof m.transfer === 'string' &&
          TRANSFER.test(m.transfer) &&
          (IMAGE_FORMATS as readonly string[]).includes(m.format as string) &&
          int(m.width, 1, LIMITS.imageSide) &&
          int(m.height, 1, LIMITS.imageSide) &&
          m.width * m.height <= LIMITS.imagePixels &&
          int(m.size, 1, LIMITS.imageBytes) &&
          typeof m.sha256 === 'string' &&
          SHA256.test(m.sha256)
        );
      case 'clipboard-sync':
        return m.state === 'on' || m.state === 'off';
      case 'file-resume-offer':
        return (
          typeof m.transfer === 'string' &&
          TRANSFER.test(m.transfer) &&
          typeof m.previous === 'string' &&
          TRANSFER.test(m.previous) &&
          m.previous !== m.transfer &&
          safeFilename(m.name) &&
          int(m.size, 1, LIMITS.fileBytes) &&
          typeof m.sha256 === 'string' &&
          SHA256.test(m.sha256)
        );
      case 'file-resume-accept':
        return (
          typeof m.transfer === 'string' &&
          TRANSFER.test(m.transfer) &&
          int(m.from, 0, Math.ceil(LIMITS.fileBytes / LIMITS.chunkBytes))
        );
      case 'input-state':
        return (
          int(m.epoch, 1, LOCAL_YIELD.maxEpoch) &&
          ((m.state === 'yielded' && (YIELD_REASONS as readonly unknown[]).includes(m.reason)) ||
            (m.state === 'ready' && m.reason === null))
        );
      case 'input-barrier':
        return int(m.epoch, 0, LOCAL_YIELD.maxEpoch) && time(m.t);
    }
  })();
  if (!ok) {
    if (kind === 'clipboard-text') throw new ControlError('too-large');
    if (
      kind === 'clipboard-image' &&
      typeof m.format === 'string' &&
      !IMAGE_FORMATS.includes(m.format as 'image/png')
    )
      throw new ControlError('unsupported');
    throw new ControlError('invalid-message');
  }
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

/**
 * Receiver rule for profile 2 input age (docs/51 §4.4), the reference for native endpoints.
 * `t` is the sender's monotonic milliseconds since it created this peer connection; clocks of
 * the two computers are never compared. The receiver keeps the smallest `(arrival − t)` seen on
 * this peer as its baseline; an event whose `(arrival − t)` exceeds the baseline by more than
 * `maxAgeMs` was delayed in transit (a stalled network, a resumed laptop, a queued SCTP buffer)
 * and is dropped. `t` going backwards is a replay. Releases (`down: false`, `release-all`) are
 * applied even when stale, so a held key or button is never left pressed. A new peer
 * connection starts a new guard.
 */
export function createInputAgeGuard(maxAgeMs: number = LIMITS.inputMaxAgeMs) {
  let baseline = Number.POSITIVE_INFINITY;
  let lastT = -1;
  return (t: number, arrival: number, release: boolean): void => {
    if (!Number.isSafeInteger(t) || t < 0) throw new ControlError('invalid-message');
    if (t < lastT) throw new ControlError('replay');
    lastT = t;
    const delay = arrival - t;
    if (delay < baseline) baseline = delay;
    if (!release && delay - baseline > maxAgeMs) throw new ControlError('stale-input');
  };
}
