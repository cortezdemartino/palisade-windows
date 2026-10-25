'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { Writable } = require('node:stream');
const { AssistLease } = require('../src/assist/lease.cjs');
const { mapPoint, InputLedger, SHORTCUTS } = require('../src/assist/input-model.cjs');
const { fileMetadata, clipboardPayload, ClipboardWindow, receiveFile } = require('../src/assist/data-model.cjs');
function leaseFixture() {
  const binding = { sessionId: 'session', hostAccount: '@host:test', hostDevice: 'h', guestAccount: '@guest:test', guestDevice: 'g', endpointId: 'endpoint' };
  const grant = { ...binding, grantId: 'abcdefghijklmnop', nonce: 'qrstuvwxyz123456', generation: 1,
    issuedAt: 1000, expiresAt: 901000, capabilities: ['control_mouse', 'control_keyboard', 'file_receive'] };
  let clock = 1000, released = 0;
  const lease = new AssistLease({ verifyGrant: value => value === 'verified-fixture' ? grant : null,
    now: () => clock, onRevoke: () => released++ });
  const message = { sessionId: binding.sessionId, grantId: grant.grantId, nonce: grant.nonce, generation: 1, sequence: 0 };
  return { lease, binding, grant, message, sessionDeadline: 901000,
    clock: value => { clock = value; }, released: () => released };
}
test('isolated admission requires independently verified exact binding and separate local consent', () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  assert.throws(() => h.lease.check('control_mouse', h.message));
  h.lease.consentTo(['control_mouse']); h.lease.check('control_mouse', h.message);
  assert.throws(() => h.lease.check('control_keyboard', h.message));
  assert.throws(() => h.lease.check('control_mouse', h.message));
  for (const field of ['sessionId', 'grantId', 'nonce', 'generation']) assert.throws(() => h.lease.check('control_mouse', { ...h.message, sequence: 1, [field]: 'forged' }));
  const bad = leaseFixture(); assert.throws(() => bad.lease.bind('verified-fixture', { ...bad.binding, guestDevice: 'other' }, bad.sessionDeadline));
  const forged = leaseFixture(); assert.throws(() => forged.lease.bind('unverified', forged.binding, forged.sessionDeadline));
});
test('expiry, clock rollback, emergency revocation and floods terminate the lease', () => {
  for (const reason of ['expiry', 'clock', 'stop', 'flood']) {
    const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline); h.lease.consentTo(['control_mouse']);
    if (reason === 'expiry') h.clock(h.grant.expiresAt);
    if (reason === 'clock') h.clock(999);
    if (reason === 'stop') h.lease.revoke();
    if (reason === 'flood') for (let i = 0; i < 120; i++) h.lease.check('control_mouse', { ...h.message, sequence: i });
    assert.throws(() => h.lease.check('control_mouse', { ...h.message, sequence: 121 }));
    assert.equal(h.released(), 1); assert.equal(h.lease.state, 'terminated');
    assert.throws(() => h.lease.bind('verified-fixture', h.binding, h.sessionDeadline));
  }
});
test('coordinate planning handles content rectangles, negative monitor origins and rotation; rejects letterbox points', () => {
  const view = { x: 100, y: 50, width: 800, height: 450 }, source = { x: -1920, y: 0, width: 1920, height: 1080, rotation: 0 };
  assert.deepEqual(mapPoint({ x: 100, y: 50 }, view, source), { x: -1920, y: 0 });
  assert.deepEqual(mapPoint({ x: 900, y: 500 }, view, source), { x: -1, y: 1079 });
  assert.deepEqual(mapPoint({ x: 100, y: 50 }, view, { ...source, rotation: 90 }), { x: -1920, y: 1079 });
  for (const point of [{ x: 99, y: 50 }, { x: NaN, y: 10 }, { x: 100, y: Infinity }]) assert.throws(() => mapPoint(point, view, source));
});
test('key/button planning emits complete cleanup and refuses unapproved protected combinations', () => {
  const ledger = new InputLedger(); ledger.key('ControlLeft', true); ledger.key('KeyC', true); ledger.button('left', true);
  assert.equal(ledger.releaseAll().length, 3); assert.deepEqual(ledger.releaseAll(), []);
  ledger.key('ControlLeft', true); ledger.key('AltRight', true);
  assert.throws(() => ledger.key('Delete', true)); assert.throws(() => ledger.key('MetaLeft', true));
  assert.throws(() => ledger.key('Tab', true)); assert.equal(SHORTCUTS.copy.join('+'), 'ControlLeft+KeyC');
});
test('clipboard envelopes enforce format/size and session-local deduplication without OS access', () => {
  const window = new ClipboardWindow(); assert.throws(() => window.offer('text/plain', Buffer.from('hello')));
  window.enable(); const bytes = Buffer.from('こんにちは'); assert.equal(window.offer('text/plain', bytes).bytes.toString(), 'こんにちは');
  assert.equal(window.offer('text/plain', bytes), null); window.revoke(); assert.throws(() => window.offer('text/plain', bytes));
  for (const [format, payload] of [['text/html', bytes], ['text/plain', Buffer.from([0xff])], ['text/plain', Buffer.alloc(4194305)], ['image/png', bytes]]) assert.throws(() => clipboardPayload(format, payload));
});
test('file metadata rejects traversal, reserved names, alternate streams and quota violations', () => {
  const valid = { name: 'report.txt', size: 2, sha256: 'a'.repeat(64) };
  assert.equal(fileMetadata(valid).name, 'report.txt');
  for (const name of ['../file', 'C:\\file', 'file:stream', 'CON.txt', 'LPT1', 'nul', 'file.', 'file ', '..', 'x\u202eexe']) assert.throws(() => fileMetadata({ ...valid, name }));
  assert.throws(() => fileMetadata({ ...valid, size: 1024 ** 3 + 1 }));
});
test('bounded file receiver verifies length/hash, rechecks authorization and cancels without committing paths', async () => {
  const bytes = Buffer.from('fixture payload'), metadata = { name: 'fixture.txt', size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
  const sink = () => new Writable({ write(_chunk, _encoding, next) { next(); } });
  const signal = new AbortController().signal;
  const result = await receiveFile({ metadata, chunks: [bytes], destination: sink(), signal, authorize() { return true; } });
  assert.equal(result.sha256, metadata.sha256);
  await assert.rejects(receiveFile({ metadata, chunks: [Buffer.from('tampered')], destination: sink(), signal, authorize() { return true; } }), /integrity/);
  await assert.rejects(receiveFile({ metadata, chunks: [bytes], destination: sink(), signal, authorize() { throw new Error('revoked'); } }), /revoked/);
  const abort = new AbortController(); abort.abort();
  await assert.rejects(receiveFile({ metadata, chunks: [bytes], destination: sink(), signal: abort.signal, authorize() { return true; } }), /cancelled/);
});
test('stalled transfers cancel promptly and missing positive authorization never writes a chunk', async () => {
  const metadata = { name: 'empty.txt', size: 0, sha256: createHash('sha256').digest('hex') };
  const abort = new AbortController();
  const destination = new Writable({ write(_chunk, _encoding, next) { next(); } });
  const chunks = { [Symbol.asyncIterator]: () => ({ next: () => new Promise(() => {}) }) };
  const pending = receiveFile({ metadata, chunks, destination, signal: abort.signal, authorize: () => true });
  abort.abort(); await assert.rejects(pending, /cancelled/); assert.equal(destination.destroyed, true);
  await assert.rejects(receiveFile({ metadata, chunks: [], destination: new Writable({ write() {} }),
    signal: new AbortController().signal, authorize() {} }), /not authorized/);
});

test('lease admission requires the original session deadline and rejects grants that extend it', () => {
  for (const deadline of [undefined, null, NaN, Infinity, 1000, 900000]) {
    const h = leaseFixture();
    assert.throws(() => h.lease.bind('verified-fixture', h.binding, deadline));
    assert.equal(h.lease.state, 'terminated');
  }
});

test('expired or invalid clocks cannot record consent or retain authorization', () => {
  for (const time of [901000, 999, NaN, Infinity]) {
    const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
    h.clock(time);
    assert.throws(() => h.lease.consentTo(['control_mouse']));
    assert.equal(h.lease.state, 'terminated');
    assert.equal(h.lease.signal.aborted, true);
  }
  assert.throws(() => new AssistLease({ verifyGrant: () => null, now: () => NaN }));
});

test('local stop during verification cannot reactivate a terminated lease', () => {
  const h = leaseFixture();
  h.lease.verify = () => { h.lease.revoke(); return h.grant; };
  assert.throws(() => h.lease.bind('verified-fixture', h.binding, h.sessionDeadline));
  assert.equal(h.lease.state, 'terminated');
});

test('capability revocation aborts its tasks without restoring or revoking unrelated authority', () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  h.lease.consentTo(['control_mouse', 'file_receive']);
  const files = h.lease.signalFor('file_receive'), mouse = h.lease.signalFor('control_mouse');
  h.lease.revokeCapabilities(['file_receive']);
  assert.equal(files.aborted, true); assert.equal(mouse.aborted, false);
  assert.equal(h.lease.signal.aborted, false);
  assert.throws(() => h.lease.assertAuthorized('file_receive'));
  assert.throws(() => h.lease.signalFor('file_receive'));
  assert.throws(() => h.lease.consentTo(['file_receive']));
  h.lease.check('control_mouse', h.message);
  h.lease.revokeCapabilities(['file_receive']);
  h.lease.revoke(); assert.equal(mouse.aborted, true); assert.equal(h.released(), 1);
});

test('idle expiration aborts admitted work without waiting for another message', t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  h.lease.consentTo(['file_receive']); const signal = h.lease.signalFor('file_receive');
  t.mock.timers.tick(900000);
  assert.equal(h.lease.state, 'terminated'); assert.equal(signal.aborted, true);
  assert.equal(h.released(), 1);
});

test('authority is invalidated and cancellation signalled before a failing cleanup callback', () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  h.lease.consentTo(['file_receive']); const signal = h.lease.signalFor('file_receive');
  let called = 0;
  h.lease.onRevoke = () => {
    called++; assert.equal(h.lease.state, 'terminated'); assert.equal(signal.aborted, true);
    assert.equal(h.lease.signal.aborted, true); throw new Error('cleanup failed');
  };
  assert.throws(() => h.lease.revoke(), /cleanup failed/);
  assert.throws(() => h.lease.assertAuthorized('file_receive'));
  h.lease.revoke(); assert.equal(called, 1);
});

test('revoking file permission cancels a stalled receive while mouse permission stays valid', async () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  h.lease.consentTo(['file_receive', 'control_mouse']);
  let returned = false;
  const chunks = { [Symbol.asyncIterator]: () => ({ next: () => new Promise(() => {}),
    return: () => { returned = true; return Promise.resolve({ done: true }); } }) };
  const destination = new Writable({ write(_chunk, _encoding, next) { next(); } });
  const pending = receiveFile({ metadata: { name: 'empty.txt', size: 0, sha256: createHash('sha256').digest('hex') },
    chunks, destination, signal: h.lease.signalFor('file_receive'),
    authorize: () => { h.lease.assertAuthorized('file_receive'); return true; } });
  h.lease.revokeCapabilities(['file_receive']);
  await assert.rejects(pending, /cancelled/); assert.equal(destination.destroyed, true);
  assert.equal(returned, true); h.lease.check('control_mouse', h.message); h.lease.revoke();
});

test('unauthorized file receive refuses before opening or polling its source iterator', async () => {
  let opened = false;
  const chunks = { [Symbol.asyncIterator]: () => { opened = true; return { next: () => new Promise(() => {}) }; } };
  const pending = receiveFile({ metadata: { name: 'empty.txt', size: 0, sha256: createHash('sha256').digest('hex') },
    chunks, destination: new Writable({ write() {} }), signal: new AbortController().signal, authorize: () => false });
  assert.equal(opened, false);
  await assert.rejects(pending, /not authorized/);
});

test('all requested permissions are invalidated before scoped cancellation listeners run', () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  h.lease.consentTo(['control_mouse', 'file_receive']);
  let checked = false;
  h.lease.signalFor('file_receive').addEventListener('abort', () => {
    assert.throws(() => h.lease.assertAuthorized('control_mouse')); checked = true;
  }, { once: true });
  h.lease.revokeCapabilities(['file_receive', 'control_mouse']);
  assert.equal(checked, true); h.lease.revoke();
});

test('revocation interrupts an in-flight write and does not report completed data', async () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  h.lease.consentTo(['file_receive']);
  let writing, progress = 0;
  const started = new Promise(resolve => { writing = resolve; });
  const bytes = Buffer.from('disposable transfer fixture');
  const pending = receiveFile({ metadata: { name: 'fixture.txt', size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') },
    chunks: [bytes], destination: new Writable({ write() { writing(); } }), signal: h.lease.signalFor('file_receive'),
    authorize: () => { h.lease.assertAuthorized('file_receive'); return true; }, onProgress: () => progress++ });
  await started; h.lease.revoke(); await assert.rejects(pending, /cancelled/);
  assert.equal(progress, 0);
});

test('a failed destination interrupts a stalled source instead of waiting for more chunks', async () => {
  const abort = new AbortController();
  const destination = new Writable({ write(_chunk, _encoding, next) { next(); } });
  const pending = receiveFile({ metadata: { name: 'empty.txt', size: 0, sha256: createHash('sha256').digest('hex') },
    chunks: { [Symbol.asyncIterator]: () => ({ next: () => new Promise(() => {}) }) },
    destination, signal: abort.signal, authorize: () => true });
  const rejected = assert.rejects(pending, /destination|closed|fixture/i);
  destination.destroy(new Error('fixture destination failed'));
  // A bounded watchdog prevents a broken implementation from hanging the suite.
  const timer = setTimeout(() => abort.abort(), 500);
  try { await rejected; } finally { clearTimeout(timer); }
});

test('consent and grant counters reject malformed values without expanding authority', () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  for (const consent of [[], ['control_mouse', 'control_mouse'], ['clipboard_send_text']]) {
    assert.throws(() => h.lease.consentTo(consent));
    assert.equal(h.lease.state, 'consent-pending');
  }
  h.lease.consentTo(['control_mouse']);
  assert.throws(() => h.lease.revokeCapabilities(['control_keyboard', 'unknown']));
  h.lease.check('control_mouse', h.message); h.lease.revoke();
  const unsafe = leaseFixture(); unsafe.grant.generation = Number.MAX_SAFE_INTEGER + 1;
  assert.throws(() => unsafe.lease.bind('verified-fixture', unsafe.binding, unsafe.sessionDeadline));
});

test('revocation inside authorization cannot open a source or finalize a transfer', async () => {
  const abort = new AbortController(); let opened = false;
  await assert.rejects(receiveFile({ metadata: { name: 'empty.txt', size: 0, sha256: createHash('sha256').digest('hex') },
    chunks: { [Symbol.iterator]: () => { opened = true; return [][Symbol.iterator](); } },
    destination: new Writable({ write() {} }), signal: abort.signal,
    authorize: () => { abort.abort(); return true; } }), /cancelled/);
  assert.equal(opened, false);
});

test('local permission state cannot authorize a capability absent from the verified grant', () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding, h.sessionDeadline);
  h.lease.consentTo(['control_mouse']);
  h.lease.consent.add('clipboard_send_text');
  assert.throws(() => h.lease.assertAuthorized('clipboard_send_text'));
  assert.throws(() => h.lease.check('clipboard_send_text', h.message));
  h.lease.revoke();
});
