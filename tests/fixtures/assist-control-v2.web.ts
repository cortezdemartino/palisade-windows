import {
  base64urlToBytes,
  CONTROL_PROTOCOL,
  ControlError,
  isCapability,
  NATIVE_CAPABILITIES,
  type Capability,
} from './protocol';

/**
 * Stream-bound Assist, `palisade.assist.control` **version 2** (docs/48-ASSIST-STREAM-PROTOCOL.md).
 *
 * An amendment beside version 1, not a change to it: version 1 grants, fixtures and their
 * fifteen-minute ceiling stay exactly as they were, and each version's parser refuses the other.
 * A version 2 Assist session is a child of one exact published stream (a LiveKit screen-share
 * track, identified by its server-assigned track SID, which changes on every publication). It
 * binds the publisher's account and device, the Matrix room, the helper's account, device and
 * WebRTC certificate, the capabilities, a generation and an absolute deadline fixed when the
 * publisher approved it (at most two hours). Authority to act additionally needs a fresh
 * service-signed lease (at most 20 seconds) that lists the grant: a frozen page, lost network or
 * ended stream therefore stops everything within one lease.
 *
 * Capability names are shared with version 1; in a stream session "host" means the publisher and
 * "guest" the helper.
 */

export const STREAM_VERSION = 2;

export const STREAM_LIMITS = Object.freeze({
  /** Default session length offered to the publisher. */
  defaultDurationMs: 15 * 60_000,
  /** Shortest selectable session. */
  minDurationMs: 60_000,
  /** Longest session: exactly two hours from approval is allowed, a millisecond more is not. */
  maxDurationMs: 2 * 60 * 60_000,
  /** Longest lease; the service issues 20-second leases and clients refresh every 5 seconds. */
  leaseMs: 20_000,
  /** Clock skew tolerated for "issued in the future". */
  skewMs: 30_000,
  /** Pending requests per stream, and solicitations per stream. */
  pendingRequests: 32,
  solicitations: 4,
  /** Unanswered requests and solicitations expire (and always with the stream). */
  requestTtlMs: 10 * 60_000,
});

/** Durations the publisher can choose, in minutes (any whole minute 1–120 is valid). */
export const DURATION_CHOICES_MINUTES = [5, 15, 30, 60, 90, 120] as const;

export interface StreamGrant {
  issuer: 'palisade';
  protocol: typeof CONTROL_PROTOCOL;
  version: 2;
  grantId: string;
  /** The Assist session (child of the stream). */
  session: string;
  /** Matrix room ID of the voice channel carrying the stream. */
  room: string;
  /** LiveKit track SID of the exact screen-share publication (stream incarnation). */
  stream: string;
  capability: Capability;
  generation: number;
  nonce: string;
  issuedAt: number;
  /** When the publisher approved the session; the deadline counts from here. */
  approvedAt: number;
  /** Absolute session deadline, fixed at approval: approvedAt < deadline ≤ approvedAt + 2 h. */
  sessionDeadline: number;
  /** This grant's end; never after sessionDeadline. */
  expiresAt: number;
  publisherAccount: string;
  publisherDevice: string;
  helperAccount: string;
  helperDevice: string;
  /** SHA-256 fingerprint of the helper's WebRTC certificate. */
  helperTransport: string;
  /** The publisher device's enrolled native endpoint, for native capabilities; else null. */
  endpoint: string | null;
}

export interface SignedStreamGrant {
  grant: StreamGrant;
  signature: string;
}

/** A short-lived statement that the session is live and which grants may act right now. */
export interface StreamLease {
  issuer: 'palisade';
  protocol: typeof CONTROL_PROTOCOL;
  version: 2;
  kind: 'lease';
  session: string;
  stream: string;
  /** The publisher device's generation when issued; grants with a higher one are not covered. */
  generation: number;
  /** Grants that may act until `expiresAt`, sorted. Empty when nothing is active. */
  grants: string[];
  issuedAt: number;
  /** At most 20 seconds after issuedAt, and never after the session deadline. */
  expiresAt: number;
  sessionDeadline: number;
}

export interface SignedStreamLease {
  lease: StreamLease;
  signature: string;
}

export const STREAM_GRANT_KEYS = [
  'approvedAt',
  'capability',
  'endpoint',
  'expiresAt',
  'generation',
  'grantId',
  'helperAccount',
  'helperDevice',
  'helperTransport',
  'issuedAt',
  'issuer',
  'nonce',
  'protocol',
  'publisherAccount',
  'publisherDevice',
  'room',
  'session',
  'sessionDeadline',
  'stream',
  'version',
] as const;

export const STREAM_LEASE_KEYS = [
  'expiresAt',
  'generation',
  'grants',
  'issuedAt',
  'issuer',
  'kind',
  'protocol',
  'session',
  'sessionDeadline',
  'stream',
  'version',
] as const;

const ID = /^[A-Za-z0-9_-]{8,128}$/;
const NONCE = /^[A-Za-z0-9_-]{16,128}$/;
const MXID = /^@[^\s:]{1,255}:[^\s]{1,255}$/;
/** Matrix room ID; room version 12 IDs have no server part. */
const ROOM = /^![A-Za-z0-9._~\-+/=]{1,255}(:[A-Za-z0-9.\-:[\]]+)?$/;
const TRACK = /^TR_[A-Za-z0-9]{4,64}$/;
const DEVICE = /^[A-Za-z0-9_-]{1,64}$/;
const FINGERPRINT = /^sha-256 (?:[0-9A-F]{2}:){31}[0-9A-F]{2}$/;
const SIGNATURE = /^[A-Za-z0-9_-]{86}$/;
const MAX_TIME = 8_640_000_000_000_000; // the largest valid JavaScript Date

function plain(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const proto = Object.getPrototypeOf(value) as unknown;
  return proto === Object.prototype || proto === null;
}

function exactKeys(record: Record<string, unknown>, keys: readonly string[]): boolean {
  const own = Object.keys(record).sort();
  return own.length === keys.length && [...keys].sort().every((key, i) => own[i] === key);
}

function time(value: unknown): value is number {
  return (
    typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= MAX_TIME
  );
}

function header(value: Record<string, unknown>): void {
  if (value.issuer !== 'palisade' || value.protocol !== CONTROL_PROTOCOL) {
    throw new ControlError('incompatible-protocol');
  }
  if (value.version !== STREAM_VERSION) throw new ControlError('incompatible-protocol');
}

/**
 * A duration the publisher may choose: a whole number of milliseconds, positive, at least one
 * minute and at most exactly two hours. Anything else (fractions, NaN, overflow) is refused.
 */
export function validDuration(ms: unknown): ms is number {
  return (
    typeof ms === 'number' &&
    Number.isSafeInteger(ms) &&
    ms >= STREAM_LIMITS.minDurationMs &&
    ms <= STREAM_LIMITS.maxDurationMs
  );
}

/** Validates a version 2 grant's shape and internal consistency (not its signature). */
export function parseStreamGrant(value: unknown): StreamGrant {
  if (!plain(value)) throw new ControlError('invalid-message');
  // Version first, so a version 1 grant is reported as incompatible rather than malformed.
  if ('version' in value && value.version !== STREAM_VERSION && value.protocol === CONTROL_PROTOCOL)
    throw new ControlError('incompatible-protocol');
  if (!exactKeys(value, STREAM_GRANT_KEYS)) throw new ControlError('invalid-message');
  header(value);
  const g = value;
  const ok =
    typeof g.grantId === 'string' &&
    ID.test(g.grantId) &&
    typeof g.session === 'string' &&
    ID.test(g.session) &&
    typeof g.room === 'string' &&
    ROOM.test(g.room) &&
    typeof g.stream === 'string' &&
    TRACK.test(g.stream) &&
    isCapability(g.capability) &&
    typeof g.generation === 'number' &&
    Number.isSafeInteger(g.generation) &&
    g.generation >= 1 &&
    typeof g.nonce === 'string' &&
    NONCE.test(g.nonce) &&
    time(g.issuedAt) &&
    time(g.approvedAt) &&
    time(g.sessionDeadline) &&
    time(g.expiresAt) &&
    typeof g.publisherAccount === 'string' &&
    MXID.test(g.publisherAccount) &&
    typeof g.helperAccount === 'string' &&
    MXID.test(g.helperAccount) &&
    g.publisherAccount !== g.helperAccount &&
    typeof g.publisherDevice === 'string' &&
    DEVICE.test(g.publisherDevice) &&
    typeof g.helperDevice === 'string' &&
    DEVICE.test(g.helperDevice) &&
    typeof g.helperTransport === 'string' &&
    FINGERPRINT.test(g.helperTransport) &&
    (g.endpoint === null || (typeof g.endpoint === 'string' && ID.test(g.endpoint)));
  if (!ok) throw new ControlError('invalid-message');
  const approvedAt = g.approvedAt as number;
  const deadline = g.sessionDeadline as number;
  const issuedAt = g.issuedAt as number;
  const expiresAt = g.expiresAt as number;
  if (!validDuration(deadline - approvedAt)) throw new ControlError('invalid-message');
  if (issuedAt < approvedAt || expiresAt <= issuedAt || expiresAt > deadline) {
    throw new ControlError('invalid-message');
  }
  const native = NATIVE_CAPABILITIES.includes(g.capability as Capability);
  if (native !== (g.endpoint !== null)) throw new ControlError('invalid-message');
  return g as unknown as StreamGrant;
}

/** Validates a lease's shape and consistency (not its signature). */
export function parseStreamLease(value: unknown): StreamLease {
  if (!plain(value)) throw new ControlError('invalid-message');
  if ('version' in value && value.version !== STREAM_VERSION && value.protocol === CONTROL_PROTOCOL)
    throw new ControlError('incompatible-protocol');
  if (!exactKeys(value, STREAM_LEASE_KEYS)) throw new ControlError('invalid-message');
  header(value);
  const l = value;
  const grants = l.grants;
  const ok =
    l.kind === 'lease' &&
    typeof l.session === 'string' &&
    ID.test(l.session) &&
    typeof l.stream === 'string' &&
    TRACK.test(l.stream) &&
    typeof l.generation === 'number' &&
    Number.isSafeInteger(l.generation) &&
    l.generation >= 1 &&
    Array.isArray(grants) &&
    grants.length <= 8 &&
    grants.every(
      (id, i) => typeof id === 'string' && ID.test(id) && (i === 0 || grants[i - 1] < id),
    ) &&
    time(l.issuedAt) &&
    time(l.expiresAt) &&
    time(l.sessionDeadline);
  if (!ok) throw new ControlError('invalid-message');
  const issuedAt = l.issuedAt as number;
  const expiresAt = l.expiresAt as number;
  if (
    expiresAt <= issuedAt ||
    expiresAt - issuedAt > STREAM_LIMITS.leaseMs ||
    expiresAt > (l.sessionDeadline as number)
  ) {
    throw new ControlError('invalid-message');
  }
  return l as unknown as StreamLease;
}

function canonical(record: Record<string, unknown>, keys: readonly string[]): Uint8Array {
  const ordered: Record<string, unknown> = {};
  for (const key of keys) ordered[key] = record[key];
  return new TextEncoder().encode(JSON.stringify(ordered));
}

/** Signed bytes: the 20 keys in sorted order, compact JSON, UTF-8. */
export function canonicalStreamGrant(grant: StreamGrant): Uint8Array {
  return canonical(grant as unknown as Record<string, unknown>, STREAM_GRANT_KEYS);
}

/** Signed bytes: the 11 keys in sorted order, compact JSON, UTF-8. */
export function canonicalStreamLease(lease: StreamLease): Uint8Array {
  return canonical(lease as unknown as Record<string, unknown>, STREAM_LEASE_KEYS);
}

async function verifySignature(
  issuerKey: Uint8Array,
  signature: unknown,
  bytes: Uint8Array,
): Promise<void> {
  if (typeof signature !== 'string' || !SIGNATURE.test(signature)) {
    throw new ControlError('invalid-message');
  }
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
    base64urlToBytes(signature).slice().buffer,
    bytes.slice().buffer,
  );
  if (!valid) throw new ControlError('binding-mismatch');
}

export interface StreamExpectation {
  session: string;
  room: string;
  stream: string;
  publisherAccount: string;
  publisherDevice: string;
  helperAccount: string;
  helperDevice: string;
  now: number;
  /**
   * The session deadline this verifier first accepted. Every later grant must carry exactly the
   * same deadline: a refreshed or reconnected grant can never move it.
   */
  sessionDeadline?: number;
  helperTransport?: string;
  /** For native capabilities: this endpoint's own ID. */
  endpoint?: string;
  minimumGeneration?: number;
}

/** Verifies a signed version 2 grant against the issuer key and the verifier's own facts. */
export async function verifyStreamGrant(
  signed: unknown,
  issuerKey: Uint8Array,
  expect: StreamExpectation,
): Promise<StreamGrant> {
  if (!plain(signed) || !exactKeys(signed, ['grant', 'signature'])) {
    throw new ControlError('invalid-message');
  }
  const grant = parseStreamGrant(signed.grant);
  await verifySignature(issuerKey, signed.signature, canonicalStreamGrant(grant));
  const bound =
    grant.session === expect.session &&
    grant.room === expect.room &&
    grant.stream === expect.stream &&
    grant.publisherAccount === expect.publisherAccount &&
    grant.publisherDevice === expect.publisherDevice &&
    grant.helperAccount === expect.helperAccount &&
    grant.helperDevice === expect.helperDevice &&
    (expect.helperTransport === undefined || grant.helperTransport === expect.helperTransport) &&
    (expect.endpoint === undefined || grant.endpoint === expect.endpoint) &&
    (expect.sessionDeadline === undefined || grant.sessionDeadline === expect.sessionDeadline);
  if (!bound) throw new ControlError('binding-mismatch');
  if (grant.expiresAt <= expect.now || grant.issuedAt > expect.now + STREAM_LIMITS.skewMs) {
    throw new ControlError('expired');
  }
  if (expect.minimumGeneration !== undefined && grant.generation < expect.minimumGeneration) {
    throw new ControlError('stale-generation');
  }
  return grant;
}

/** Verifies a signed lease for this session and stream at `now`. */
export async function verifyStreamLease(
  signed: unknown,
  issuerKey: Uint8Array,
  expect: { session: string; stream: string; sessionDeadline: number; now: number },
): Promise<StreamLease> {
  if (!plain(signed) || !exactKeys(signed, ['lease', 'signature'])) {
    throw new ControlError('invalid-message');
  }
  const lease = parseStreamLease(signed.lease);
  await verifySignature(issuerKey, signed.signature, canonicalStreamLease(lease));
  if (
    lease.session !== expect.session ||
    lease.stream !== expect.stream ||
    lease.sessionDeadline !== expect.sessionDeadline
  ) {
    throw new ControlError('binding-mismatch');
  }
  if (lease.expiresAt <= expect.now || lease.issuedAt > expect.now + STREAM_LIMITS.skewMs) {
    throw new ControlError('expired');
  }
  return lease;
}

/**
 * May this grant act now? Only while a verified, unexpired lease lists it, covers its generation
 * and the session deadline has not passed.
 */
export function leaseCovers(lease: StreamLease, grant: StreamGrant, now: number): boolean {
  return (
    lease.session === grant.session &&
    lease.stream === grant.stream &&
    lease.sessionDeadline === grant.sessionDeadline &&
    lease.grants.includes(grant.grantId) &&
    grant.generation <= lease.generation &&
    now < lease.expiresAt &&
    now < grant.expiresAt
  );
}
