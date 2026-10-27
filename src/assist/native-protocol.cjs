'use strict';
// Canonical native bridge v1; provenance and executable conformance live under tests/fixtures.
const NATIVE_PROTOCOL = 'palisade.assist.native';
const NATIVE_VERSION = 1;
const NATIVE_LIMITS = Object.freeze({
    messageChars: 32_768,
    sdpChars: 16_384,
    candidateChars: 1_024,
    grantChars: 4_096,
    leaseChars: 2_048,
    outstanding: 8,
    requestTimeoutMs: 120_000
});
const NATIVE_ERRORS = [
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
    'internal'
];
class NativeBridgeError extends Error {
    code;
    constructor(code){
        super(code), this.code = code;
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
function plain(value) {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const proto = Object.getPrototypeOf(value);
    return proto === Object.prototype || proto === null;
}
function exact(r, keys) {
    const own = Object.keys(r).sort();
    return own.length === keys.length && [
        ...keys
    ].sort().every((k, i)=>own[i] === k);
}
const str = (v, re)=>typeof v === 'string' && re.test(v);
const bounded = (v, max)=>typeof v === 'string' && v.length > 0 && v.length <= max;
const REQUESTS = {
    hello: {
        versions: (v)=>Array.isArray(v) && v.length >= 1 && v.length <= 4 && v.every((x)=>Number.isSafeInteger(x) && x >= 1)
    },
    'enroll.key': {
        origin: (v)=>str(v, ORIGIN)
    },
    'enroll.prove': {
        endpointId: (v)=>str(v, ID),
        challenge: (v)=>str(v, B64_32),
        account: (v)=>str(v, MXID),
        device: (v)=>str(v, DEVICE)
    },
    'grant.offer': {
        session: (v)=>str(v, ID),
        grant: (v)=>bounded(v, NATIVE_LIMITS.grantChars)
    },
    'peer.offer': {
        session: (v)=>str(v, ID),
        grantId: (v)=>str(v, ID),
        sdp: (v)=>bounded(v, NATIVE_LIMITS.sdpChars)
    },
    'peer.candidate': {
        session: (v)=>str(v, ID),
        candidate: (v)=>typeof v === 'string' && v.length <= NATIVE_LIMITS.candidateChars,
        mid: (v)=>v === null || str(v, MID)
    },
    'peer.close': {
        session: (v)=>str(v, ID)
    },
    'capture.start': {
        operation: (v)=>str(v, ID),
        room: (v)=>str(v, ROOM)
    },
    'capture.bind': {
        operation: (v)=>str(v, ID),
        trackSid: (v)=>str(v, TRACK)
    },
    'capture.cancel': {
        operation: (v)=>str(v, ID)
    },
    'capture.stop': {
        operation: (v)=>str(v, ID)
    },
    status: {},
    stop: {}
};
const NATIVE_OPS = Object.keys(REQUESTS);
function parseJson(raw) {
    if (typeof raw !== 'string') return raw;
    if (raw.length > NATIVE_LIMITS.messageChars) throw new NativeBridgeError('invalid-message');
    try {
        return JSON.parse(raw);
    } catch  {
        throw new NativeBridgeError('invalid-message');
    }
}
function envelope(m) {
    if (m.p !== NATIVE_PROTOCOL) throw new NativeBridgeError('invalid-message');
    if (m.v !== NATIVE_VERSION) throw new NativeBridgeError('unsupported-version');
}
function parseRequest(raw) {
    const m = parseJson(raw);
    if (!plain(m)) throw new NativeBridgeError('invalid-message');
    envelope(m);
    if (!str(m.id, REQUEST_ID) || typeof m.op !== 'string' || !Object.hasOwn(REQUESTS, m.op)) {
        throw new NativeBridgeError('invalid-message');
    }
    const fields = REQUESTS[m.op];
    if (!exact(m, [
        'p',
        'v',
        'id',
        'op',
        ...Object.keys(fields)
    ])) {
        throw new NativeBridgeError('invalid-message');
    }
    for (const [k, check] of Object.entries(fields)){
        if (!check(m[k])) throw new NativeBridgeError('invalid-message');
    }
    return m;
}
const RESULTS = {
    hello: {
        version: (v)=>v === NATIVE_VERSION,
        features: (v)=>plain(v) && exact(v, [
                'capture',
                'clipboard',
                'files',
                'input'
            ]) && Object.values(v).every((x)=>typeof x === 'boolean')
    },
    'enroll.key': {
        publicKey: (v)=>str(v, B64_32),
        issuerFingerprint: (v)=>str(v, HEX_SHA256),
        pin: (v)=>v === 'release' || v === 'confirmed'
    },
    'enroll.prove': {
        signature: (v)=>str(v, SIGNATURE)
    },
    'grant.offer': {
        signature: (v)=>str(v, SIGNATURE)
    },
    'peer.offer': {
        sdp: (v)=>bounded(v, NATIVE_LIMITS.sdpChars),
        transportSignature: (v)=>str(v, SIGNATURE)
    },
    'peer.candidate': {},
    'peer.close': {},
    'capture.start': {
        operation: (v)=>str(v, ID)
    },
    'capture.bind': {},
    'capture.cancel': {},
    'capture.stop': {},
    status: {
        endpoint: (v)=>plain(v) && exact(v, [
                'endpointId',
                'enrolled',
                'pin'
            ]) && typeof v.enrolled === 'boolean' && (v.endpointId === null || str(v.endpointId, ID)) && (v.pin === 'none' || v.pin === 'release' || v.pin === 'confirmed' || v.pin === 'mismatch'),
        capture: (v)=>plain(v) && exact(v, [
                'operation',
                'state',
                'trackSid'
            ]) && (v.operation === null || str(v.operation, ID)) && CAPTURE_STATES.includes(v.state) && (v.trackSid === null || str(v.trackSid, TRACK)),
        controller: (v)=>plain(v) && exact(v, [
                'session',
                'state'
            ]) && (v.session === null || str(v.session, ID)) && CONTROLLER_STATES.includes(v.state)
    },
    stop: {}
};
const CAPTURE_STATES = [
    'idle',
    'picking',
    'live',
    'stopped',
    'cancelled',
    'failed'
];
const CONTROLLER_STATES = [
    'idle',
    'consent',
    'active',
    'suspended'
];
function parseResponse(raw, op) {
    const m = parseJson(raw);
    if (!plain(m)) throw new NativeBridgeError('invalid-message');
    envelope(m);
    if (!str(m.id, REQUEST_ID)) throw new NativeBridgeError('invalid-message');
    if (m.ok === false) {
        if (!exact(m, [
            'p',
            'v',
            'id',
            'ok',
            'error'
        ])) throw new NativeBridgeError('invalid-message');
        if (!NATIVE_ERRORS.includes(m.error)) {
            throw new NativeBridgeError('invalid-message');
        }
        return m;
    }
    if (m.ok !== true || !exact(m, [
        'p',
        'v',
        'id',
        'ok',
        'result'
    ]) || !plain(m.result)) {
        throw new NativeBridgeError('invalid-message');
    }
    const fields = RESULTS[op];
    if (!exact(m.result, Object.keys(fields))) throw new NativeBridgeError('invalid-message');
    for (const [k, check] of Object.entries(fields)){
        if (!check(m.result[k])) throw new NativeBridgeError('invalid-message');
    }
    return m;
}
const EVENTS = {
    'peer.candidate': {
        session: (v)=>str(v, ID),
        candidate: (v)=>typeof v === 'string' && v.length <= NATIVE_LIMITS.candidateChars,
        mid: (v)=>v === null || str(v, MID)
    },
    'capture.status': {
        operation: (v)=>str(v, ID),
        state: (v)=>CAPTURE_STATES.includes(v),
        reason: (v)=>v === null || typeof v === 'string' && [
                'picker-cancelled',
                'source-lost',
                'track-ended',
                'local-stop',
                'reload',
                'replaced',
                'denied',
                'error'
            ].includes(v)
    },
    lifecycle: {
        session: (v)=>v === null || str(v, ID),
        state: (v)=>CONTROLLER_STATES.includes(v) || v === 'stopped',
        reason: (v)=>v === null || typeof v === 'string' && [
                'local-stop',
                'lease-lost',
                'revoked',
                'expired',
                'lock',
                'sleep',
                'desktop-change',
                'source-change',
                'replaced',
                'error'
            ].includes(v)
    }
};
function parseEvent(raw) {
    const m = parseJson(raw);
    if (!plain(m)) throw new NativeBridgeError('invalid-message');
    envelope(m);
    if (typeof m.event !== 'string' || !Object.hasOwn(EVENTS, m.event)) {
        throw new NativeBridgeError('invalid-message');
    }
    if (typeof m.seq !== 'number' || !Number.isSafeInteger(m.seq) || m.seq < 1) {
        throw new NativeBridgeError('invalid-message');
    }
    const fields = EVENTS[m.event];
    if (!exact(m, [
        'p',
        'v',
        'event',
        'seq',
        ...Object.keys(fields)
    ])) {
        throw new NativeBridgeError('invalid-message');
    }
    for (const [k, check] of Object.entries(fields)){
        if (!check(m[k])) throw new NativeBridgeError('invalid-message');
    }
    return m;
}
const enc = new TextEncoder();
function endpointTransportStatement(session, endpointId, endpointFingerprint, helperTransport) {
    return enc.encode(`palisade.assist.control/endpoint-transport/v2\n${session}\n${endpointId}\n${endpointFingerprint}\n${helperTransport}`);
}
const CAPTURE_ELEMENT_ATTRIBUTE = 'data-palisade-assist-capture';

module.exports=Object.freeze({NATIVE_PROTOCOL,NATIVE_VERSION,NATIVE_LIMITS,NATIVE_ERRORS,NativeBridgeError,NATIVE_OPS,parseRequest,CAPTURE_STATES,CONTROLLER_STATES,parseResponse,parseEvent,endpointTransportStatement,CAPTURE_ELEMENT_ATTRIBUTE});
