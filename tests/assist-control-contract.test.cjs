'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash, generateKeyPairSync } = require('node:crypto');
const { stripTypeScriptTypes } = require('node:module');
const { AssistLease } = require('../src/assist/lease.cjs');
const directory = path.join(__dirname, 'fixtures');
const provenance = require('./fixtures/assist-control-v1.provenance.json');
const fixtures = require('./fixtures/assist-control-v1.json');
const rawKey = Uint8Array.from(Buffer.from(fixtures.issuerPublicKey, 'base64url'));
const canonicalSource = fs.readFileSync(path.join(directory, 'assist-control-v1.web.ts'), 'utf8').replace(/\r\n/g, '\n');
const compiled = stripTypeScriptTypes(canonicalSource, { mode: 'transform' });
const contract = import('data:text/javascript;base64,' + Buffer.from(compiled).toString('base64'));

test('canonical control v1 source and fixtures retain their exact companion provenance', () => {
  assert.equal(provenance.commit, 'ee9c2763f19e62b405bbf9a9c97f187bc5a1291a');
  for (const entry of provenance.files) {
    const bytes = fs.readFileSync(path.join(directory, entry.local), 'utf8').replace(/\r\n/g, '\n');
    assert.equal(createHash('sha256').update(bytes).digest('hex'), entry.sha256, entry.source);
  }
  assert.equal(rawKey.length, 32);
});

test('real canonical Ed25519 verification accepts each published positive grant fixture', async () => {
  const api = await contract;
  for (const entry of fixtures.grants.valid) {
    const grant = await api.verifyGrant(entry.signed, rawKey, fixtures.expect);
    assert.equal(grant.grantId, entry.signed.grant.grantId, entry.name);
  }
});

test('canonical grant verification rejects tampering, untrusted bindings and expired fixtures', async () => {
  const api = await contract;
  for (const entry of fixtures.grants.invalid) {
    await assert.rejects(api.verifyGrant(entry.signed, rawKey, fixtures.expect),
      error => error.code === entry.error, entry.name);
  }
});

test('canonical message fixtures preserve exact rejection codes and accepted shapes', async () => {
  const api = await contract;
  for (const message of fixtures.messages.valid) assert.deepEqual(api.parseControlMessage(message), message);
  for (const entry of fixtures.messages.invalid) {
    assert.throws(() => api.parseControlMessage(entry.message), error => error.code === entry.error, entry.name);
  }
});

test('legacy control v1 cannot admit two-hour or unversioned stream grants', async () => {
  const api = await contract, original = fixtures.grants.valid[0].signed.grant;
  assert.throws(() => api.parseGrant({ ...original, expiresAt: original.issuedAt + 2 * 60 * 60_000 }),
    error => error.code === 'invalid-message');
  assert.throws(() => api.parseGrant({ ...original, streamIncarnation: 'new-stream' }),
    error => error.code === 'invalid-message');
  assert.throws(() => api.parseGrant({ ...original, version: 2 }),
    error => error.code === 'incompatible-protocol');
});

test('a real canonical native grant still requires matching local binding and consent', async () => {
  const api = await contract;
  const entry = fixtures.grants.valid.find(value => value.signed.grant.capability === 'control_mouse');
  const grant = await api.verifyGrant(entry.signed, rawKey, fixtures.expect);
  // Test-only normalization, not a production bridge or native issuer/verifier.
  const local = Object.freeze({ sessionId: grant.session, hostAccount: grant.hostAccount,
    hostDevice: grant.hostDevice, guestAccount: grant.guestAccount, guestDevice: grant.guestDevice,
    endpointId: grant.endpoint, grantId: grant.grantId, generation: grant.generation, nonce: grant.nonce,
    issuedAt: grant.issuedAt, expiresAt: grant.expiresAt, capabilities: Object.freeze([grant.capability]) });
  const expected = { sessionId: fixtures.expect.session, hostAccount: fixtures.expect.hostAccount,
    hostDevice: fixtures.expect.hostDevice, guestAccount: fixtures.expect.guestAccount,
    guestDevice: fixtures.expect.guestDevice, endpointId: 'endpoint_fixture_01' };
  const make = () => new AssistLease({ now: () => fixtures.expect.now,
    verifyGrant: value => value === local ? local : null });
  const lease = make(); lease.bind(local, expected, fixtures.expect.sessionDeadline);
  const message = { sessionId: grant.session, grantId: grant.grantId, nonce: grant.nonce,
    generation: grant.generation, sequence: 0 };
  assert.throws(() => lease.check('control_mouse', message));
  lease.consentTo(['control_mouse']); lease.check('control_mouse', message);
  assert.throws(() => lease.check('control_keyboard', { ...message, sequence: 1 }));
  lease.revoke(); assert.equal(lease.signal.aborted, true);
  const wrong = make();
  assert.throws(() => wrong.bind(local, { ...expected, endpointId: 'another-endpoint' }, fixtures.expect.sessionDeadline));
  assert.equal(wrong.state, 'terminated');
});

test('another real Ed25519 issuer key cannot validate a canonical signed grant', async () => {
  const api = await contract;
  const { publicKey } = generateKeyPairSync('ed25519');
  const wrong = Uint8Array.from(publicKey.export({ type: 'spki', format: 'der' }).subarray(-32));
  await assert.rejects(api.verifyGrant(fixtures.grants.valid[0].signed, wrong, fixtures.expect),
    error => error.code === 'binding-mismatch');
});

test('canonical directions keep guest input distinct from host clipboard and file traffic', async () => {
  const api = await contract;
  assert.equal(api.capabilityFor('pointer-move', 'host'), undefined);
  assert.equal(api.capabilityFor('pointer-move', 'guest'), 'control_mouse');
  assert.equal(api.capabilityFor('clipboard-text', 'guest'), 'clipboard_to_host');
  assert.equal(api.capabilityFor('clipboard-text', 'host'), 'clipboard_to_guest');
  assert.equal(api.capabilityFor('file-offer', 'guest'), 'file_to_host');
  assert.equal(api.capabilityFor('file-offer', 'host'), 'file_to_guest');
});
