'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { generateKeyPairSync, sign, createHash } = require('node:crypto');
const { mkdtemp, writeFile, readFile, rm, readdir } = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { fingerprint, transitionBytes, resolveTrust } = require('../src/updates/key-trust.cjs');
const { normalisePayload, signedBytes, verifyEnvelope, verifyDownloadedFile, releaseAssetUrl, fetchRelease, RELEASES_API } = require('../src/updates/release.cjs');
const { downloadInstaller } = require('../src/updates/downloader.cjs');

const root = generateKeyPairSync('ed25519');
const next = generateKeyPairSync('ed25519');
const future = generateKeyPairSync('ed25519');
const pem = (key) => key.export({ type: 'spki', format: 'pem' });
const bytes = Buffer.from('trusted Windows installer fixture');
function transition(from, to, minVersion) {
  const payload = { schema: 1, from: fingerprint(from.publicKey), toPublicKey: pem(to.publicKey), minVersion };
  return { payload, signature: sign(null, transitionBytes(payload), from.privateKey).toString('base64') };
}
const first = transition(root, next, '0.2.2');
const second = transition(next, future, '0.3.0');
function manifest(version = '0.2.2') {
  const filename = `Palisade-Setup-${version}-x64.exe`;
  return normalisePayload({ schema: 3, channel: 'stable', version, filename,
    downloadUrl: releaseAssetUrl(version, filename), size: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex'),
    sha512: createHash('sha512').update(bytes).digest('base64'), publishedAt: '2026-10-17T12:00:00.000Z' });
}
function envelope(payload, key = next, transitions = [first]) {
  return { payload, signature: sign(null, signedBytes(payload), key.privateKey).toString('base64'), transitions };
}

test('successor signatures require a transition authenticated by the original trust anchor', () => {
  assert.deepEqual(verifyEnvelope(envelope(manifest()), pem(root.publicKey)), manifest());
  assert.throws(() => verifyEnvelope(envelope(manifest(), next, []), pem(root.publicKey)));
  assert.throws(() => verifyEnvelope(envelope(manifest(), future), pem(root.publicKey)));
  assert.throws(() => verifyEnvelope({ ...envelope(manifest()), signature: '' }, pem(root.publicKey)));
  const forged = transition(future, next, '0.2.2');
  assert.throws(() => verifyEnvelope(envelope(manifest(), next, [forged]), pem(root.publicKey)));
});

test('transition tampering, cycles, wrong sequence and premature activation fail closed', () => {
  for (const change of [{ minVersion: '0.2.1' }, { toPublicKey: pem(future.publicKey) }, { from: fingerprint(future.publicKey) }]) {
    assert.throws(() => resolveTrust(pem(root.publicKey), [{ ...first, payload: { ...first.payload, ...change } }], '0.3.0'));
  }
  assert.throws(() => resolveTrust(pem(root.publicKey), [first], '0.2.1'));
  assert.throws(() => resolveTrust(pem(root.publicKey), [second, first], '0.3.0'));
  assert.throws(() => resolveTrust(pem(root.publicKey), [first, transition(next, root, '0.3.0')], '0.3.0'));
  assert.throws(() => resolveTrust(pem(root.publicKey), [first, transition(next, future, '0.2.2')], '0.3.0'));
  assert.throws(() => resolveTrust(pem(root.publicKey), Array(9).fill(first), '0.3.0'));
});

test('installed transition history cannot be removed, replaced or rolled back to an ancestor key', () => {
  const current = envelope(manifest('0.3.0'), future, [first, second]);
  assert.deepEqual(verifyEnvelope(current, pem(root.publicKey), [first]), manifest('0.3.0'));
  assert.throws(() => verifyEnvelope(envelope(manifest('0.3.1'), root, []), pem(root.publicKey), [first]));
  assert.throws(() => verifyEnvelope(envelope(manifest('0.3.1')), pem(root.publicKey), [first, second]));
  const alternate = transition(root, future, '0.2.2');
  assert.throws(() => verifyEnvelope(envelope(manifest('0.3.1'), future, [alternate]), pem(root.publicKey), [first]));
});

test('signed release binds the SHA-256, version and exact GitHub download location', () => {
  const signed = envelope(manifest());
  for (const change of [{ sha256: '0'.repeat(64) }, { size: bytes.length + 1 },
    { version: '0.2.3', filename: 'Palisade-Setup-0.2.3-x64.exe', downloadUrl: releaseAssetUrl('0.2.3', 'Palisade-Setup-0.2.3-x64.exe') }]) {
    assert.throws(() => verifyEnvelope({ ...signed, payload: { ...signed.payload, ...change } }, pem(root.publicKey)));
  }
  for (const downloadUrl of ['https://evil.test/installer.exe', signed.payload.downloadUrl + '?redirect=1', signed.payload.downloadUrl.replace('https:', 'http:')]) {
    assert.throws(() => normalisePayload({ ...signed.payload, downloadUrl }));
  }
});

test('v3 download checks both digests and refuses tampered installers', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'palisade-v3-'));
  const payload = manifest();
  try {
    const response = (content) => async () => ({ ok: true, status: 200, body: (async function* () { yield content; })() });
    const file = await downloadInstaller(payload, dir, response(bytes));
    assert.equal(await verifyDownloadedFile(file, payload), true);
    await writeFile(file, Buffer.alloc(bytes.length, 0));
    assert.equal(await verifyDownloadedFile(file, payload), false);
    await rm(file);
    await assert.rejects(downloadInstaller(payload, dir, response(Buffer.alloc(bytes.length, 0))));
    assert.deepEqual(await readdir(dir), []);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('authenticated successor cannot authorize a release below the installed version', async () => {
  let manifestRequests = 0;
  const response = (value, url) => ({ ok: true, url, headers: new Map(), body: (async function* () { yield Buffer.from(JSON.stringify(value)); })() });
  const fetchFn = async (url) => {
    if (url === RELEASES_API) return response({ tag_name: 'v0.2.2', draft: false, prerelease: false,
      assets: [{ name: 'release.json' }, { name: manifest().filename }] }, url);
    manifestRequests++;
    return response(envelope(manifest()), url);
  };
  assert.equal(await fetchRelease(fetchFn, pem(root.publicKey), '0.3.0', [first]), null);
  assert.equal(manifestRequests, 0);
  assert.deepEqual(await fetchRelease(fetchFn, pem(root.publicKey), '0.2.1'), manifest());
});

test('manual test-client bootstrap pins the supplied SPKI Ed25519 key', async () => {
  const old = await readFile(path.join(__dirname, '../assets/update-public-key.pem'));
  assert.match(old.toString(), /^-----BEGIN PUBLIC KEY-----/);
  assert.equal(fingerprint(old), '59337b8e1ba2145ff688c6674f5c09c633cc9c2b80e8ee57eab82eded3eaa781');
  resolveTrust(old, JSON.parse(await readFile(path.join(__dirname, '../assets/update-key-transitions.json'))), require('../package.json').version);
});
