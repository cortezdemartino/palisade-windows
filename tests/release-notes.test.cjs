'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { generateKeyPairSync, sign, createHash } = require('node:crypto');
const { mkdtemp, readFile, writeFile, rm } = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { signedBytes, normalisePayload, releaseAssetUrl } = require('../src/updates/release.cjs');
const { normaliseDraft, bindDraft, notesBytes, verifyNotes, noteView, NOTES_ASSET } = require('../src/updates/notes.cjs');
const { createNotesHistory } = require('../src/updates/notes-history.cjs');
const { transitionBytes, fingerprint } = require('../src/updates/key-trust.cjs');
const root = generateKeyPairSync('ed25519');
const draft = { schema: 1, version: '0.4.0', locale: 'en', title: '', summary: 'A reviewed release summary.',
  sections: [{ category: 'fixed', items: ['A specific verified fix.'] }], compatibility: ['Windows x64.'] };
function pair(version = draft.version, key = root, transitions = []) {
  const payload = normalisePayload({ schema: 3, channel: 'stable', version,
    filename: `Palisade-Setup-${version}-x64.exe`, size: 1, sha256: createHash('sha256').update('x').digest('hex'),
    sha512: createHash('sha512').update('x').digest('base64'), publishedAt: '2026-11-10T00:00:00.000Z',
    downloadUrl: releaseAssetUrl(version, `Palisade-Setup-${version}-x64.exe`) });
  const release = { payload, signature: sign(null, signedBytes(payload), key.privateKey).toString('base64'), transitions };
  const notesPayload = bindDraft({ ...draft, version }, payload);
  const notes = { payload: notesPayload, signature: sign(null, notesBytes(notesPayload, payload), key.privateKey).toString('base64') };
  return { release, notes };
}
const valid = pair();
test('notes are domain-separated and bound to the exact authenticated v3 release', () => {
  assert.equal(verifyNotes(valid.notes, valid.release, root.publicKey).summary, draft.summary);
  assert.match(notesBytes(valid.notes.payload, valid.release.payload).toString(), /^PALISADE-DESKTOP-RELEASE-NOTES-V1\n/);
  assert.equal(noteView(valid.notes.payload).sections[0].title, 'Fixed');
  assert.equal(releaseAssetUrl('0.4.0', NOTES_ASSET).endsWith('/v0.4.0/release-notes-v1.json'), true);
});
test('notes reject missing signatures, foreign keys, cross-release substitution and tampering', () => {
  for (const notes of [{ payload: valid.notes.payload }, { ...valid.notes, signature: '' },
    { ...valid.notes, payload: { ...valid.notes.payload, summary: 'Changed prose.' } },
    { ...valid.notes, extra: true }, pair('0.4.0', generateKeyPairSync('ed25519')).notes,
    pair('0.5.0').notes]) {
    assert.throws(() => verifyNotes(notes, valid.release, root.publicKey));
  }
  for (const field of ['product', 'platform', 'channel', 'publishedAt', 'releaseSha256']) {
    assert.throws(() => verifyNotes({ ...valid.notes, payload: { ...valid.notes.payload, [field]: 'wrong' } }, valid.release, root.publicKey));
  }
  const changedInstaller = { ...valid.release.payload, sha256: '0'.repeat(64) };
  const anotherRelease = { ...valid.release, payload: changedInstaller,
    signature: sign(null, signedBytes(changedInstaller), root.privateKey).toString('base64') };
  assert.throws(() => verifyNotes(valid.notes, anotherRelease, root.publicKey), /match/);
});
test('editorial validation bounds categories, characters, locales, fields and versions', () => {
  for (const delta of [{ version: '0.4.0-beta' }, { version: '00.4.0' }, { schema: 2 }, { extra: true },
    { locale: '../../en' }, { title: 'a'.repeat(121) }, { summary: '<script>bad</script>' },
    { summary: 'https://evil.example/' }, { summary: '[open](file:x)' }, { summary: 'a\u202eb' },
    { summary: '\ud800' }, { summary: 'a\nb' }, { summary: ' untrimmed' },
    { sections: [{ category: 'unknown', items: ['Bad.'] }] },
    { sections: [...draft.sections, ...draft.sections] }, { sections: [{ category: 'new', items: [] }] },
    { compatibility: Array(7).fill('Too many.') }]) {
    assert.throws(() => normaliseDraft({ ...draft, ...delta }));
  }
  assert.throws(() => normaliseDraft(draft, '0.4.1'));
  assert.throws(() => bindDraft(draft, { ...valid.release.payload, schema: 2 }));
  assert.deepEqual(normaliseDraft({ ...draft, sections: [] }).sections, []);
});
test('an authenticated successor signs notes; a shortened packaged transition chain is refused', () => {
  const successor = generateKeyPairSync('ed25519');
  const payload = { schema: 1, from: fingerprint(root.publicKey),
    toPublicKey: successor.publicKey.export({ type: 'spki', format: 'pem' }), minVersion: '0.4.0' };
  const chain = [{ payload, signature: sign(null, transitionBytes(payload), root.privateKey).toString('base64') }];
  const rotated = pair('0.4.0', successor, chain);
  assert.equal(verifyNotes(rotated.notes, rotated.release, root.publicKey, chain).version, '0.4.0');
  assert.throws(() => verifyNotes(valid.notes, valid.release, root.publicKey, chain));
  assert.throws(() => verifyNotes(valid.notes, rotated.release, root.publicKey, chain));
});
test('display history preserves pre-rotation notes but refuses post-rotation root-only notes', async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'palisade-rotated-notes-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const successor = generateKeyPairSync('ed25519');
  const payload = { schema: 1, from: fingerprint(root.publicKey), minVersion: '0.4.0',
    toPublicKey: successor.publicKey.export({ type: 'spki', format: 'pem' }) };
  const chain = [{ payload, signature: sign(null, transitionBytes(payload), root.privateKey).toString('base64') }];
  const history = createNotesHistory({ directory, root: root.publicKey, transitions: chain });
  const historical = pair('0.3.0');
  await history.add(historical.release, historical.notes);
  await assert.rejects(history.add(valid.release, valid.notes));
  const rotated = pair('0.4.0', successor, chain);
  await history.add(rotated.release, rotated.notes);
  assert.deepEqual((await history.load()).map((note) => note.version), ['0.4.0', '0.3.0']);
});
test('offline notes cache is bounded, reverified, sorted and has no unsigned fallback', async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'palisade-notes-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const history = createNotesHistory({ directory, root: root.publicKey });
  assert.deepEqual(await history.load(), []);
  await history.add(valid.release, valid.notes);
  const newer = pair('0.5.0');
  await history.add(newer.release, newer.notes);
  const reload = createNotesHistory({ directory, root: root.publicKey });
  assert.deepEqual((await reload.load()).map((note) => note.version), ['0.5.0', '0.4.0']);
  const cache = JSON.parse(await readFile(path.join(directory, 'release-notes-v1.json'), 'utf8'));
  cache[0].notes.payload.summary = 'Cache tampering.';
  await writeFile(path.join(directory, 'release-notes-v1.json'), JSON.stringify(cache));
  assert.deepEqual((await reload.load()).map((note) => note.version), ['0.4.0']);
  await writeFile(path.join(directory, 'release-notes-v1.json'), 'x'.repeat(1024 * 1024 + 1));
  assert.deepEqual(await reload.load(), []);
});
test('missing or unsigned online notes never enter the signed history', async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'palisade-notes-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  for (const value of [{ unsigned: 'GitHub prose' }, { ...valid.notes, signature: '' }]) {
    const history = createNotesHistory({ directory, root: root.publicKey, fetchFn: async (url) => {
      assert.equal(url, releaseAssetUrl('0.4.0', NOTES_ASSET));
      return new Response(JSON.stringify(value));
    } });
    await assert.rejects(history.fetchFor(valid.release));
    assert.deepEqual(history.snapshot(), []);
  }
});
