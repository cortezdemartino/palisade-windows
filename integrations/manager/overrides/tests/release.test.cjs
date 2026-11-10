'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { generateKeyPairSync, sign, createHash } = require('node:crypto');
const { mkdtemp, rm, readFile, writeFile } = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const {
  RELEASES_API, compareVersions, normalisePayload, signedBytes, verifyEnvelope,
  releaseAssetUrl, fetchAsset, fetchRelease, verifyDownloadedFile,
} = require('../src/updates/release.cjs');
const { download } = require('../src/updates/service.cjs');
const downloadInstaller = (manifest, destination, fetchFn) => download(manifest, destination, new AbortController().signal, () => {}, fetchFn);

const { privateKey, publicKey } = generateKeyPairSync('ed25519');
const goodContent = Buffer.from('trusted installer content');
const sha512 = createHash('sha512').update(goodContent).digest('base64');
const payload = normalisePayload({ schema: 1, channel: 'stable', version: '0.3.0',
  filename: 'Palisade-Setup-0.3.0-x64.exe', sha512, size: goodContent.length,
  publishedAt: '2026-10-15T10:00:00.000Z' });
const envelope = { payload, signature: sign(null, signedBytes(payload), privateKey).toString('base64') };
const key = publicKey.export({ type: 'spki', format: 'pem' });

function response(value, url, status = 200) {
  const data = Buffer.isBuffer(value) ? value : Buffer.from(JSON.stringify(value));
  return { ok: status >= 200 && status < 300, status, url,
    headers: new Map([['content-length', `${data.length}`]]),
    body: (async function*() { yield data; })() };
}

function latest(version = '0.3.0') {
  return { draft: false, prerelease: false, tag_name: `v${version}`, assets: [
    { name: 'release.json' }, { name: `Palisade-Setup-${version}-x64.exe` },
  ] };
}

test('signed manifest accepts the pinned release key', () => {
  assert.deepEqual(verifyEnvelope(envelope, key), payload);
  assert.equal(signedBytes(payload).toString().startsWith('PALISADE-DESKTOP-RELEASE-V1\n'), true);
});

test('tampering and substituted signing keys are rejected', () => {
  for (const field of ['version', 'size', 'sha512', 'publishedAt', 'filename']) {
    const modified = { ...payload, [field]: field === 'size' ? 26 :
      field === 'version' ? '0.4.0' : field === 'sha512' ? createHash('sha512').update('bad').digest('base64') :
        field === 'publishedAt' ? '2026-10-15T10:00:01.000Z' : 'other.exe' };
    assert.throws(() => verifyEnvelope({ ...envelope, payload: modified }, key));
  }
  assert.throws(() => verifyEnvelope(envelope, generateKeyPairSync('ed25519').publicKey));
  assert.throws(() => verifyEnvelope({ ...envelope, signature: '' }, key));
  assert.throws(() => verifyEnvelope({ ...envelope, unexpected: 1 }, key));
});

test('signed payload rejects unsafe filenames, versions and invalid fields', () => {
  for (const delta of [{ filename: '../other.exe' }, { schema: 2 }, { channel: 'dev' },
    { size: -5 }, { version: '1.0.0-alpha' }, { version: '../../1' },
    { publishedAt: 'now' }, { extra: true }]) {
    assert.throws(() => normalisePayload({ ...payload, ...delta }));
  }
  assert.equal(compareVersions('0.10.0', '0.9.8'), 1);
  assert.equal(compareVersions('0.3.0', '0.3.0'), 0);
  assert.equal(compareVersions('0.2.0', '0.3.0'), -1);
});

test('release URLs cannot be controlled by the manifest', () => {
  assert.equal(releaseAssetUrl('0.3.0', 'release.json'),
    'https://github.com/cortezdemartino/palisade-windows/releases/download/v0.3.0/release.json');
  assert.throws(() => releaseAssetUrl('0.3.0', 'evil.exe'));
  assert.throws(() => releaseAssetUrl('0.3.0', '../release.json'));
});

test('GitHub release metadata and manifest must match', async () => {
  const modern = normalisePayload({ ...payload, schema: 3,
    sha256: createHash('sha256').update(goodContent).digest('hex'),
    downloadUrl: releaseAssetUrl(payload.version, payload.filename) });
  const modernEnvelope = { payload: modern, transitions: [], signature: sign(null, signedBytes(modern), privateKey).toString('base64') };
  const download = releaseAssetUrl(payload.version, 'release-v3.json');
  const fetchFn = async (url) => {
    if (url === RELEASES_API) return response(latest(), url);
    if (url === download) return response(modernEnvelope, url);
    throw new Error('Unexpected URL');
  };
  assert.deepEqual(await fetchRelease(fetchFn, key, '0.2.0'), modern);
  assert.equal(await fetchRelease(fetchFn, key, '0.3.0'), null);
  await assert.rejects(fetchRelease(async (url) => url === RELEASES_API
    ? response(latest('0.4.0'), url) : response(envelope, url), key, '0.2.0'));
  await assert.rejects(fetchRelease(async (url) => url === RELEASES_API
    ? response({ ...latest(), prerelease: true }, url) : response(envelope, url), key, '0.2.0'));
  await assert.rejects(fetchRelease(async (url) => url === RELEASES_API
    ? response({ ...latest(), assets: [] }, url) : response(envelope, url), key, '0.2.0'));
});

test('release redirects only to known HTTPS GitHub asset hosts', async () => {
  const original = releaseAssetUrl(payload.version, 'release.json');
  const allowed = 'https://release-assets.githubusercontent.com/1234/asset';
  const fetchFn = async (url) => url === original
    ? { status: 302, headers: new Map([['location', allowed]]) }
    : response(envelope, url);
  const found = await fetchAsset(original, { fetchFn });
  assert.equal(found.url, allowed);
  await assert.rejects(fetchAsset(original, { fetchFn: async () => ({ status: 302,
    headers: new Map([['location', 'https://other.example.com/installer.exe']]) }) }));
  await assert.rejects(fetchAsset(original, { fetchFn: async () => ({ status: 302,
    headers: new Map([['location', 'http://github.com/bad']]) }) }));
});

test('installer download verifies full length and SHA-512 before saving', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'palisade-test-'));
  const fileUrl = releaseAssetUrl(payload.version, payload.filename);
  try {
    const file = await downloadInstaller(payload, dir, async (url) => {
      assert.equal(url, fileUrl);
      return response(goodContent, url);
    });
    assert.deepEqual(await readFile(file), goodContent);
    assert.equal(await verifyDownloadedFile(file, payload), true);
    await writeFile(file, 'tampered content');
    assert.equal(await verifyDownloadedFile(file, payload), false);
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('truncated downloads and bad hashes leave no executable', async () => {
  for (const bytes of [Buffer.from('short'), Buffer.from('tampered installer content')]) {
    const dir = await mkdtemp(path.join(os.tmpdir(), 'palisade-test-'));
    try {
      await assert.rejects(downloadInstaller(payload, dir, async (url) => response(bytes, url)));
      await assert.rejects(readFile(path.join(dir, payload.filename)));
    } finally { await rm(dir, { recursive: true, force: true }); }
  }
});

test('published 0.2.2 signature verifies with the synchronized manager key and rejects tampering', async () => {
 const manifest = JSON.parse(await readFile(path.join(__dirname, 'fixtures/release-0.2.2.json'), 'utf8'));
 const pinned = await readFile(path.join(__dirname, '../assets/update-public-key.pem'));
 assert.equal(verifyEnvelope(manifest, pinned).version, '0.2.2');
 assert.throws(() => verifyEnvelope({...manifest, payload: {...manifest.payload, size: manifest.payload.size + 1}}, pinned));
});
