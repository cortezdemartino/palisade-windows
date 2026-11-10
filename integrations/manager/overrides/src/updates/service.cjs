'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const release = require('./release.cjs');
const { NOTES_ASSET, NOTES_LIMIT, verifyNotes, noteView } = require('./notes.cjs');
const { downloadInstaller } = require('./downloader.cjs');
const packagedTransitions = require('../../assets/update-key-transitions.json');
async function retry(task, signal) {
  for (let attempt = 0; ; attempt++) {
    signal?.throwIfAborted();
    try { return await task(); } catch (e) {
      if (signal?.aborted || attempt === 2) throw e;
      await require('node:timers/promises').setTimeout(500 * 2 ** attempt, undefined, { signal });
    }
  }
}
async function discover(key, signal, fetchFn = fetch, transitions = packagedTransitions, { notesTimeoutMs = 10000 } = {}) {
  if (!Number.isInteger(notesTimeoutMs) || notesTimeoutMs < 1 || notesTimeoutMs > 10000) throw Error('Invalid notes deadline.');
  const callerSignal = signal;
  signal = AbortSignal.any([signal, AbortSignal.timeout(60000)].filter(Boolean));
  const response = await retry(async () => {
    const result = await fetchFn(release.RELEASES_API, { redirect: 'error', signal: AbortSignal.any([signal, AbortSignal.timeout(15000)]),
      headers: { 'User-Agent': 'Palisade-Installer', Accept: 'application/vnd.github+json' } });
    if (result.status === 429 || result.status >= 500) {
      await result.body?.cancel?.();
      throw new Error('The update service is temporarily unavailable. Try again later.');
    }
    return result;
  }, signal);
  if (response.status === 404) throw new Error('No stable Palisade release is currently published. Use a signed offline package or check again later.');
  const info = await release.readLimited(response, 256 * 1024);
  if (!info || info.draft !== false || info.prerelease !== false || typeof info.tag_name !== 'string' || !/^v/.test(info.tag_name)) throw new Error('The update service did not return a stable release.');
  const version = info.tag_name.slice(1); release.compareVersions(version, '0.0.0');
  let envelope;
  try {
    const response = await release.fetchAsset(release.releaseAssetUrl(version, 'release-v3.json'), { fetchFn, signal });
    envelope = await release.readLimited(response, 32768);
    if (envelope?.payload?.schema !== 3) throw Error('Invalid current release schema.');
  } catch (error) {
    // Missing historical metadata can fall back; invalid modern signatures never do.
    if (error.status !== 404 || transitions.length) throw error;
    envelope = await release.readLimited(await release.fetchAsset(release.releaseAssetUrl(version, 'release.json'), { fetchFn, signal }), 16384);
    if (envelope?.payload?.schema !== 1) throw Error('Invalid legacy release schema.');
  }
  const manifest = release.verifyEnvelope(envelope, key, transitions);
  if (manifest.version !== version) throw new Error('The signed version does not match the release.');
  let notesPair, notesView;
  if (manifest.schema === 3) {
    try {
      const notesSignal = AbortSignal.any([callerSignal, AbortSignal.timeout(notesTimeoutMs)].filter(Boolean));
      const notes = await release.readLimited(await release.fetchAsset(release.releaseAssetUrl(version, NOTES_ASSET), { fetchFn, signal: notesSignal }), NOTES_LIMIT);
      notesView = noteView(verifyNotes(notes, envelope, key, transitions));
      notesPair = { release: envelope, notes };
    } catch { /* Missing/invalid notes cannot authorize or block an authenticated installer. */ }
  }
  callerSignal?.throwIfAborted();
  return { manifest, notes: notesView?.summary || 'Verified release notes are unavailable.', notesPair, source: 'online' };
}
function enforceVersion(manifest, installed, highest, architecture = process.arch) {
  if (architecture !== 'x64') throw new Error('This package supports Windows x64 only.');
  for (const floor of [installed, highest].filter(Boolean)) {
    if (release.compareVersions(manifest.version, floor) < 0) throw new Error('This package is older than the installed or previously verified version. Downgrades are blocked.');
  }
}
async function offline(directory, key, transitions = packagedTransitions) {
  let file = path.join(directory, 'release-v3.json');
  let modern = true;
  try { await fs.access(file); } catch (error) {
    if (error.code !== 'ENOENT' || transitions.length) throw error;
    modern = false; file = path.join(directory, 'release.json');
  }
  const envelope = await release.readJsonFileLimited(file, 32768);
  if (envelope?.payload?.schema !== (modern ? 3 : 1)) throw Error('Invalid offline release schema.');
  const manifest = release.verifyEnvelope(envelope, key, transitions);
  const packagePath = path.join(directory, manifest.filename);
  if (!(await release.verifyDownloadedFile(packagePath, manifest))) throw new Error('The offline installer failed verification.');
  let notesPair, notesView;
  if (modern) try {
    const file = path.join(directory, NOTES_ASSET);
    const notes = await release.readJsonFileLimited(file, NOTES_LIMIT);
    notesView = noteView(verifyNotes(notes, envelope, key, transitions));
    notesPair = { release: envelope, notes };
  } catch { /* Installer authority is independent of editorial availability. */ }
  return { manifest, source: 'offline', packagePath, notesPair,
    notes: notesView?.summary || 'Verified release notes are unavailable.' };
}
async function download(manifest, destination, signal, progress, fetchFn = fetch) {
  return downloadInstaller(manifest, destination, fetchFn, progress, { signal });
}
module.exports = { discover, offline, download, enforceVersion, retry };
