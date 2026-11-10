'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { generateKeyPairSync, sign, createHash } = require('node:crypto');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { discover, offline, download } = require('../src/updates/service.cjs');
const { normalisePayload, signedBytes, releaseAssetUrl, RELEASES_API } = require('../src/updates/release.cjs');
const { bindDraft, notesBytes } = require('../src/updates/notes.cjs');
const key = generateKeyPairSync('ed25519');
const data = Buffer.from('disposable installer bytes');
const payload = normalisePayload({ schema: 3, channel: 'stable', version: '0.4.0',
  filename: 'Palisade-Setup-0.4.0-x64.exe', size: data.length,
  sha256: createHash('sha256').update(data).digest('hex'), sha512: createHash('sha512').update(data).digest('base64'),
  publishedAt: '2026-11-10T00:00:00.000Z', downloadUrl: releaseAssetUrl('0.4.0', 'Palisade-Setup-0.4.0-x64.exe') });
const release = { payload, transitions: [], signature: sign(null, signedBytes(payload), key.privateKey).toString('base64') };
const editorial = bindDraft({ schema: 1, version: payload.version, locale: 'en', title: '',
  summary: 'Authenticated editorial fixture.', sections: [], compatibility: [] }, payload);
const notes = { payload: editorial, signature: sign(null, notesBytes(editorial, payload), key.privateKey).toString('base64') };
const response = (value) => new Response(JSON.stringify(value));
const metadata = { tag_name: 'v0.4.0', draft: false, prerelease: false, body: 'Unsigned release prose.' };

test('server errors, malformed modern metadata and a packaged rotation floor never fall back to legacy', async () => {
  for (const mode of ['server', 'malformed', 'wrong-schema', 'rotation-floor']) {
    let legacy = false;
    const fetchFn = async (url) => {
      if (url === RELEASES_API) return response(metadata);
      if (url.endsWith('/release-v3.json')) {
        if (mode === 'server') return new Response('', { status: 503 });
        if (mode === 'malformed') return new Response('{invalid');
        if (mode === 'wrong-schema') return response({ ...release, payload: { ...payload, schema: 1 } });
        return new Response('', { status: 404 });
      }
      legacy = true; throw Error('Unexpected legacy fallback.');
    };
    await assert.rejects(discover(key.publicKey, new AbortController().signal, fetchFn,
      mode === 'rotation-floor' ? [{ payload: { minVersion: '0.4.0' } }] : []));
    assert.equal(legacy, false);
  }
});

test('a stalled optional notes request cannot suppress a verified installer offer; caller cancel remains authoritative', async () => {
  const keepAlive = setTimeout(() => {}, 1000);
  try {
    for (const cancel of [false, true]) {
      const abort = new AbortController();
      const fetchFn = async (url, { signal }) => {
        if (url === RELEASES_API) return response(metadata);
        if (url.endsWith('/release-v3.json')) return response(release);
        if (cancel) abort.abort();
        signal.throwIfAborted();
        return new Promise((_resolve, reject) => signal.addEventListener('abort', () => reject(signal.reason), { once: true }));
      };
      const result = discover(key.publicKey, abort.signal, fetchFn, [], { notesTimeoutMs: 5 });
      if (cancel) await assert.rejects(result);
      else assert.equal((await result).manifest.version, payload.version);
    }
  } finally { clearTimeout(keepAlive); }
});

test('online Manager verifies v3 and signed notes, never falls back on invalid modern metadata', async () => {
  for (const invalid of [false, true]) {
    let legacyFetched = false;
    const fetchFn = async (url) => {
      if (url === RELEASES_API) return response(metadata);
      if (url.endsWith('/release-v3.json')) return response(invalid ? { ...release, signature: 'A'.repeat(86) + '==' } : release);
      if (url.endsWith('/release-notes-v1.json')) return response(notes);
      legacyFetched = true; throw Error('No legacy fallback permitted.');
    };
    if (invalid) await assert.rejects(discover(key.publicKey, new AbortController().signal, fetchFn));
    else {
      const offer = await discover(key.publicKey, new AbortController().signal, fetchFn);
      assert.equal(offer.notes, editorial.summary);
      assert.equal(offer.manifest.sha256, payload.sha256);
      assert.deepEqual(offer.notesPair, { release, notes });
    }
    assert.equal(legacyFetched, false);
  }
});
test('offline Manager uses the same notes and rejects tampered modern metadata and installer SHA-256', async (t) => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'manager-u1-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  await fs.writeFile(path.join(directory, 'release-v3.json'), JSON.stringify(release));
  await fs.writeFile(path.join(directory, 'release-notes-v1.json'), JSON.stringify(notes));
  await fs.writeFile(path.join(directory, payload.filename), data);
  assert.equal((await offline(directory, key.publicKey)).notes, editorial.summary);
  await fs.writeFile(path.join(directory, 'release-notes-v1.json'), JSON.stringify({ ...notes, payload: { ...editorial, summary: 'Tampered.' } }));
  assert.equal((await offline(directory, key.publicKey)).notes, 'Verified release notes are unavailable.');
  const wrongHash = { ...payload, sha256: '0'.repeat(64) };
  await assert.rejects(download(wrongHash, await fs.mkdtemp(path.join(directory, 'download-')),
    new AbortController().signal, () => {}, async () => new Response(data)), /signed manifest/);
  await fs.writeFile(path.join(directory, 'release-v3.json'), JSON.stringify({ ...release, signature: '' }));
  await assert.rejects(offline(directory, key.publicKey));
});
