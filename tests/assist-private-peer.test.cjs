'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const { operation, answer } = require('../src/assist/private-peer.cjs');
const session = 'session_fixture_0001', attempt = 'A'.repeat(43), fingerprint = 'sha-256 ' + Array(32).fill('AB').join(':');
test('trusted peer operations have closed per-action shapes and bounded packet ingress', () => {
  const values = [{ action: 'prepare', session, grantId: 'grant_fixture_0001' },
    { action: 'seal', session, attempt, ownFingerprint: fingerprint },
    { action: 'observed', session, attempt, fingerprint }, { action: 'check', session, attempt },
    { action: 'packet', session, attempt, text: 'original bytes', ingressNs: '123456789' }];
  for (const value of values) {
    assert.equal(operation(value), true);
    assert.equal(operation({ ...value, extra: true }), false);
    assert.equal(operation({ ...value, session: '../other' }), false);
    if (value.attempt) assert.equal(operation({ ...value, attempt: 'short' }), false);
  }
  for (const stamp of ['', '-1', '01', '1e9', '18446744073709551616', 123])
    assert.equal(operation({ ...values[4], ingressNs: stamp }), false);
  assert.equal(operation({ ...values[4], text: 'x'.repeat(16385) }), false);
  assert.equal(operation({ ...values[1], ownFingerprint: 'sha-256 aa' }), false);
  assert.equal(operation({ action: 'execute', session, command: 'arbitrary' }), false);
});
test('peer result vocabulary is correlated to the exact private operation', () => {
  const prepare = { outcome: 'ok', result: { attempt, helperTransport: fingerprint, profile: 'legacy', sourceOperation: 'capture_fixture_01' } };
  const seal = { outcome: 'ok', result: { transportSignature: 'B'.repeat(86) } };
  assert.equal(answer(prepare, 'prepare'), true); assert.equal(answer(prepare, 'seal'), false);
  assert.equal(answer(seal, 'seal'), true); assert.equal(answer(seal, 'observed'), false);
  assert.equal(answer({ outcome: 'ok', result: { ready: true } }, 'observed'), true);
  assert.equal(answer({ outcome: 'ok', result: {} }, 'unknown'), false);
  assert.equal(answer({ outcome: 'error', code: 'grant-invalid' }, 'prepare'), true);
  assert.equal(answer({ outcome: 'error', code: 'raw-stack-trace' }, 'prepare'), false);
  assert.equal(answer({ ...prepare, extra: true }, 'prepare'), false);
  assert.equal(answer({ outcome: 'ok', result: { ...prepare.result, profile: 'cooperative' } }, 'prepare'), false);
});
