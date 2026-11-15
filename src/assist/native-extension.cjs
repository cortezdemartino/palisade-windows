'use strict';
// Canonical native extension validation (frozen companion extension.ts).
const { NATIVE_LIMITS } = require('./native-protocol.cjs');
const EXT_PROTOCOL = 'palisade.assist.native.ext';
const EXT_VERSION = 1;
const EXT_VERSION_2 = 2;
const EXT_VERSION_3 = 3;
const EXT_ERRORS = [
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
    'internal'
];
class ExtBridgeError extends Error {
    code;
    constructor(code){
        super(code), this.code = code;
        this.name = 'ExtBridgeError';
    }
}
const ID = /^[A-Za-z0-9_-]{8,128}$/;
const REQUEST_ID = /^[A-Za-z0-9_-]{8,64}$/;
function plain(v) {
    if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
    const proto = Object.getPrototypeOf(v);
    return proto === Object.prototype || proto === null;
}
function exact(r, keys) {
    const own = Object.keys(r).sort();
    return own.length === keys.length && [
        ...keys
    ].sort().every((k, i)=>own[i] === k);
}
const id = (v)=>typeof v === 'string' && ID.test(v);
const REQUESTS = {
    'ext.hello': {
        versions: (v)=>Array.isArray(v) && v.length >= 1 && v.length <= 4 && v.every((x)=>Number.isSafeInteger(x) && x >= 1)
    },
    'resource.send-text': {
        session: id,
        grantId: id
    },
    'resource.offer-file': {
        session: id,
        grantId: id
    },
    'resource.cancel': {
        session: id,
        transfer: id
    }
};
const REQUESTS_V2 = {
    'resource.send-text': REQUESTS['resource.send-text'],
    'resource.offer-file': REQUESTS['resource.offer-file'],
    'resource.cancel': REQUESTS['resource.cancel'],
    'resource.send-image': {
        session: id,
        grantId: id
    },
    'resource.offer-files': {
        session: id,
        grantId: id
    },
    'resource.drop-surface': {
        session: id,
        grantId: id
    },
    'resource.resume': {
        session: id,
        transfer: id
    },
    'resource.sync': {
        session: id,
        grantId: id,
        state: (v)=>v === 'on' || v === 'off'
    }
};
const EXT_SAFETY_OPS = [
    'resource.cancel'
];
function isExtSafety(op, fields = {}) {
    return EXT_SAFETY_OPS.includes(op) || op === 'resource.sync' && fields.state === 'off';
}
function requestTable(version) {
    return version >= 2 ? REQUESTS_V2 : REQUESTS;
}
const RESULTS = {
    'ext.hello': {
        version: (v)=>v === EXT_VERSION,
        features: (v)=>plain(v) && exact(v, [
                'cleanupEvents',
                'clipboardText',
                'files'
            ]) && Object.values(v).every((x)=>typeof x === 'boolean')
    },
    'resource.send-text': {
        transfer: id
    },
    'resource.offer-file': {
        transfer: id
    },
    'resource.cancel': {}
};
const EXT_V2_FEATURES = [
    'cleanupEvents',
    'clipboardImage',
    'clipboardSync',
    'clipboardText',
    'dropSurface',
    'fileResume',
    'files',
    'input2',
    'multiFile'
];
const EXT_V3_FEATURES = [
    ...EXT_V2_FEATURES,
    'localYield'
];
const transferList = (v)=>Array.isArray(v) && v.length >= 1 && v.length <= 32 && v.every(id) && new Set(v).size === v.length;
const RESULTS_V2 = {
    'resource.send-text': {
        transfer: id
    },
    'resource.offer-file': {
        transfer: id
    },
    'resource.cancel': {},
    'resource.send-image': {
        transfer: id
    },
    'resource.offer-files': {
        transfers: transferList
    },
    'resource.drop-surface': {},
    'resource.resume': {
        transfer: id
    },
    'resource.sync': {}
};
function helloResult(r, max) {
    if (!exact(r, [
        'version',
        'features'
    ]) || !plain(r.features)) return false;
    if (typeof r.version !== 'number' || r.version > max) return false;
    const keys = r.version === EXT_VERSION ? [
        'cleanupEvents',
        'clipboardText',
        'files'
    ] : r.version === EXT_VERSION_2 ? [
        ...EXT_V2_FEATURES
    ] : r.version === EXT_VERSION_3 ? [
        ...EXT_V3_FEATURES
    ] : undefined;
    return keys !== undefined && exact(r.features, keys) && Object.values(r.features).every((x)=>typeof x === 'boolean');
}
const RESOURCE_STATES = [
    'awaiting-local-confirmation',
    'offered',
    'awaiting-acceptance',
    'transferring',
    'verifying',
    'complete',
    'rejected',
    'cancelled',
    'failed'
];
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
    'error'
];
const EVENTS = {
    'resource.status': {
        session: id,
        transfer: id,
        kind: (v)=>v === 'text' || v === 'file',
        direction: (v)=>v === 'to-host' || v === 'to-guest',
        state: (v)=>RESOURCE_STATES.includes(v),
        bytes: (v)=>Number.isSafeInteger(v) && v >= 0,
        total: (v)=>Number.isSafeInteger(v) && v >= 0,
        reason: (v)=>v === null || typeof v === 'string' && REASONS.includes(v)
    },
    cleanup: {
        session: (v)=>v === null || id(v),
        state: (v)=>v === 'pending' || v === 'complete',
        reason: (v)=>v === null || typeof v === 'string' && [
                'local-stop',
                'revoked',
                'expired',
                'stream-ended',
                'lease-lost',
                'quit',
                'update',
                'error'
            ].includes(v)
    }
};
const SYNC_REASONS = [
    'local-stop',
    'revoked',
    'expired',
    'stream-ended',
    'lease-lost',
    'error'
];
const EVENTS_V2 = {
    'resource.status': {
        ...EVENTS['resource.status'],
        kind: (v)=>v === 'text' || v === 'image' || v === 'file',
        resumable: (v)=>typeof v === 'boolean'
    },
    cleanup: EVENTS.cleanup,
    'sync.status': {
        session: id,
        capability: (v)=>v === 'clipboard_to_guest' || v === 'clipboard_to_host',
        state: (v)=>v === 'on' || v === 'off',
        reason: (v)=>v === null || typeof v === 'string' && SYNC_REASONS.includes(v)
    }
};
const EPOCH_MAX = 2_147_483_647;
const EXT_YIELD_REASONS = [
    'local-input',
    'local-held',
    'neutralizing'
];
const EVENTS_V3 = {
    ...EVENTS_V2,
    'input.yield': {
        session: id,
        state: (v)=>v === 'active' || v === 'yielded' || v === 'ready',
        epoch: (v)=>Number.isSafeInteger(v) && v >= 0 && v <= EPOCH_MAX,
        reason: (v)=>v === null || EXT_YIELD_REASONS.includes(v)
    }
};
function json(raw) {
    if (typeof raw !== 'string') return raw;
    if (raw.length > NATIVE_LIMITS.messageChars) throw new ExtBridgeError('invalid-message');
    try {
        return JSON.parse(raw);
    } catch  {
        throw new ExtBridgeError('invalid-message');
    }
}
function envelope(m, version, hello) {
    if (m.p !== EXT_PROTOCOL) throw new ExtBridgeError('invalid-message');
    if (m.v !== (hello ? EXT_VERSION : version)) throw new ExtBridgeError('unsupported-version');
}
function parseExtRequest(raw, version = EXT_VERSION) {
    const m = json(raw);
    if (!plain(m)) throw new ExtBridgeError('invalid-message');
    envelope(m, version, m.op === 'ext.hello');
    const table = m.op === 'ext.hello' ? REQUESTS : requestTable(version);
    if (typeof m.id !== 'string' || !REQUEST_ID.test(m.id) || typeof m.op !== 'string' || !Object.hasOwn(table, m.op)) {
        throw new ExtBridgeError('invalid-message');
    }
    const fields = table[m.op];
    if (!exact(m, [
        'p',
        'v',
        'id',
        'op',
        ...Object.keys(fields)
    ])) {
        throw new ExtBridgeError('invalid-message');
    }
    for (const [k, check] of Object.entries(fields)){
        if (!check(m[k])) throw new ExtBridgeError('invalid-message');
    }
    return m;
}
function parseExtResponse(raw, op, version = EXT_VERSION) {
    const m = json(raw);
    if (!plain(m)) throw new ExtBridgeError('invalid-message');
    envelope(m, version, op === 'ext.hello');
    if (typeof m.id !== 'string' || !REQUEST_ID.test(m.id)) throw new ExtBridgeError('invalid-message');
    if (m.ok === false) {
        if (!exact(m, [
            'p',
            'v',
            'id',
            'ok',
            'error'
        ]) || !EXT_ERRORS.includes(m.error)) {
            throw new ExtBridgeError('invalid-message');
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
        throw new ExtBridgeError('invalid-message');
    }
    if (op === 'ext.hello' && version >= EXT_VERSION_2) {
        if (!helloResult(m.result, version)) throw new ExtBridgeError('invalid-message');
        return m;
    }
    const table = version >= 2 && op !== 'ext.hello' ? RESULTS_V2 : RESULTS;
    const fields = table[op];
    if (!fields) throw new ExtBridgeError('invalid-message');
    if (!exact(m.result, Object.keys(fields))) throw new ExtBridgeError('invalid-message');
    for (const [k, check] of Object.entries(fields)){
        if (!check(m.result[k])) throw new ExtBridgeError('invalid-message');
    }
    return m;
}
function parseExtEvent(raw, version = EXT_VERSION) {
    const m = json(raw);
    if (!plain(m)) throw new ExtBridgeError('invalid-message');
    envelope(m, version, false);
    const events = version === 3 ? EVENTS_V3 : version === 2 ? EVENTS_V2 : EVENTS;
    if (typeof m.event !== 'string' || !Object.hasOwn(events, m.event)) {
        throw new ExtBridgeError('invalid-message');
    }
    if (typeof m.seq !== 'number' || !Number.isSafeInteger(m.seq) || m.seq < 1) {
        throw new ExtBridgeError('invalid-message');
    }
    const fields = events[m.event];
    if (!exact(m, [
        'p',
        'v',
        'event',
        'seq',
        ...Object.keys(fields)
    ])) {
        throw new ExtBridgeError('invalid-message');
    }
    for (const [k, check] of Object.entries(fields)){
        if (!check(m[k])) throw new ExtBridgeError('invalid-message');
    }
    return m;
}
function protocolOf(raw) {
    try {
        const m = JSON.parse(raw);
        if (!plain(m)) return undefined;
        return m.p === EXT_PROTOCOL ? 'ext' : m.p === 'palisade.assist.native' ? 'native' : undefined;
    } catch  {
        return undefined;
    }
}

module.exports = Object.freeze({EXT_PROTOCOL,EXT_VERSION,EXT_VERSION_2,EXT_VERSION_3,EXT_ERRORS,ExtBridgeError,EXT_SAFETY_OPS,isExtSafety,EXT_V2_FEATURES,EXT_V3_FEATURES,RESOURCE_STATES,EXT_YIELD_REASONS,parseExtRequest,parseExtResponse,parseExtEvent,protocolOf});
