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
  return { lease, binding, grant, message, clock: value => { clock = value; }, released: () => released };
}
test('isolated admission requires independently verified exact binding and separate local consent', () => {
  const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding);
  assert.throws(() => h.lease.check('control_mouse', h.message));
  h.lease.consentTo(['control_mouse']); h.lease.check('control_mouse', h.message);
  assert.throws(() => h.lease.check('control_keyboard', h.message));
  assert.throws(() => h.lease.check('control_mouse', h.message));
  for (const field of ['sessionId', 'grantId', 'nonce', 'generation']) assert.throws(() => h.lease.check('control_mouse', { ...h.message, sequence: 1, [field]: 'forged' }));
  const bad = leaseFixture(); assert.throws(() => bad.lease.bind('verified-fixture', { ...bad.binding, guestDevice: 'other' }));
  const forged = leaseFixture(); assert.throws(() => forged.lease.bind('unverified', forged.binding));
});
test('expiry, clock rollback, emergency revocation and floods terminate the lease', () => {
  for (const reason of ['expiry', 'clock', 'stop', 'flood']) {
    const h = leaseFixture(); h.lease.bind('verified-fixture', h.binding); h.lease.consentTo(['control_mouse']);
    if (reason === 'expiry') h.clock(h.grant.expiresAt);
    if (reason === 'clock') h.clock(999);
    if (reason === 'stop') h.lease.revoke();
    if (reason === 'flood') for (let i = 0; i < 120; i++) h.lease.check('control_mouse', { ...h.message, sequence: i });
    assert.throws(() => h.lease.check('control_mouse', { ...h.message, sequence: 121 }));
    assert.equal(h.released(), 1); assert.equal(h.lease.state, 'terminated');
    assert.throws(() => h.lease.bind('verified-fixture', h.binding));
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
