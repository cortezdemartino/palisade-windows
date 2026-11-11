'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const os = require('node:os');
const fs = require('node:fs/promises');
const { createHash } = require('node:crypto');
const { EventEmitter } = require('node:events');
const { PassThrough } = require('node:stream');
const { FILENAME, PROTOCOL, verifyHelper, exchangeApply } = require('../src/updates/native-helper.cjs');

test('a copied native updater must match the integrity record inside the app archive', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'palisade-helper-integrity-'));
  try {
    const binary = path.join(directory, FILENAME);
    const content = Buffer.from('nonexecuted fixture');
    const manifest = { schema: 1, protocol: PROTOCOL, filename: FILENAME, version: '0.3.4', size: content.length,
      sha256: createHash('sha256').update(content).digest('hex') };
    await fs.writeFile(binary, content);
    assert.equal((await verifyHelper(binary, Buffer.from(JSON.stringify(manifest)), manifest.version)).sha256, manifest.sha256);
    await assert.rejects(verifyHelper(binary, Buffer.from(JSON.stringify(manifest)), '0.3.5'), /record/);
    await fs.writeFile(binary, Buffer.alloc(content.length, 42));
    await assert.rejects(verifyHelper(binary, Buffer.from(JSON.stringify(manifest)), manifest.version), /mismatch/);
  } finally { await fs.rm(directory, { recursive: true, force: true }); }
});

function handoffFixture(options = {}) {
  const token = { stage: '75a9d45c-7191-4ddd-824f-142177570ec3', nonce: 'a'.repeat(64),
    version: '0.3.4', releaseSha256: 'b'.repeat(64) };
  const child = new EventEmitter();
  child.stdin = new PassThrough();
  child.stdout = new PassThrough();
  child.unref = () => { child.detached = true; };
  let request = '';
  child.stdin.on('data', chunk => { request += chunk.toString(); });
  const result = exchangeApply('fixture-only.exe', 123, token, { ...options, spawnFn: (_image, args, spawnOptions) => {
    assert.deepEqual(args, ['--apply', '123', token.stage, token.nonce]);
    assert.equal(spawnOptions.shell, false);
    return child;
  } });
  const send = data => child.stdout.write(JSON.stringify(data) + '\n');
  return { child, result, token, send, request: () => request };
}
test('the fixed handoff commits once after a matching ready frame and accepts only its acknowledgement', async () => {
  let commits = 0;
  const h = handoffFixture({ onCommit: () => { commits++; } });
  h.send({ state: 'ready', ...h.token });
  assert.equal(commits, 1);
  assert.equal(h.request(), 'PALISADE-UPDATE-COMMIT-V1 ' + h.token.nonce + '\n');
  h.send({ state: 'committed', stage: h.token.stage, nonce: h.token.nonce });
  await h.result;
  assert.equal(h.child.detached, true);
});
test('stale readiness and cancellation cannot send an installer authorization', async () => {
  const stale = handoffFixture();
  stale.send({ state: 'ready', ...stale.token, nonce: 'c'.repeat(64) });
  await assert.rejects(stale.result, error => error.commitSent === false);
  assert.equal(stale.request(), '');
  const abort = new AbortController();
  const canceled = handoffFixture({ signal: abort.signal });
  abort.abort();
  await assert.rejects(canceled.result, error => error.commitSent === false);
  assert.equal(canceled.request(), '');
});
test('losing acknowledgement preserves uncertain commit instead of reporting cancellation', async () => {
  const abort = new AbortController();
  const h = handoffFixture({ signal: abort.signal });
  h.send({ state: 'ready', ...h.token });
  abort.abort();
  h.child.emit('close', 1);
  await assert.rejects(h.result, error => error.commitSent === true);
});
