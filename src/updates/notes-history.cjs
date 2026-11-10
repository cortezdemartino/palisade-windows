'use strict';

const { mkdir, writeFile, rename, unlink } = require('node:fs/promises');
const { randomUUID } = require('node:crypto');
const path = require('node:path');
const { compareVersions, fetchAsset, readLimited, readJsonFileLimited, releaseAssetUrl } = require('./release.cjs');
const { NOTES_ASSET, NOTES_LIMIT, verifyNotes, noteView } = require('./notes.cjs');

const HISTORY_LIMIT = 12;
const CACHE_LIMIT = 1024 * 1024;

/** Disk is an untrusted, display-only cache. It never supplies update authority or trust anchors. */
function createNotesHistory({ directory, root, transitions = [], fetchFn = fetch }) {
  let pairs = [];
  let persistence = Promise.resolve();
  function validate(pair) {
    if (!pair || Object.keys(pair).sort().join(',') !== 'notes,release') throw new Error('Invalid notes cache entry.');
    // A historical note may predate an installed rotation. Validate only that time's required
    // packaged prefix; live update authorization always uses the complete installed prefix.
    const version = pair.release?.payload?.version;
    const prefix = transitions.filter((item) => compareVersions(item.payload.minVersion, version) <= 0);
    return verifyNotes(pair.notes, pair.release, root, prefix);
  }
  function snapshot() {
    return pairs.map((pair) => noteView(validate(pair)));
  }
  async function load() {
    try {
      const cached = await readJsonFileLimited(path.join(directory, 'release-notes-v1.json'), CACHE_LIMIT);
      if (!Array.isArray(cached) || cached.length > HISTORY_LIMIT) throw new Error('Invalid notes history.');
      const unique = new Map();
      for (const pair of cached) {
        try { const note = validate(pair); unique.set(note.version, pair); } catch { /* Omit invalid entries. */ }
      }
      pairs = [...unique.values()].sort((a, b) => compareVersions(b.release.payload.version, a.release.payload.version));
    } catch { pairs = []; }
    return snapshot();
  }
  async function persist() {
    const bytes = JSON.stringify(pairs);
    if (Buffer.byteLength(bytes) > CACHE_LIMIT) throw new Error('Notes cache too large.');
    await mkdir(directory, { recursive: true });
    const temporary = path.join(directory, `notes-${randomUUID()}.tmp`);
    try {
      await writeFile(temporary, bytes, { flag: 'wx', mode: 0o600 });
      await rename(temporary, path.join(directory, 'release-notes-v1.json'));
    } finally { await unlink(temporary).catch(() => {}); }
  }
  async function add(release, notes) {
    const pair = JSON.parse(JSON.stringify({ release, notes }));
    const value = validate(pair);
    pairs = [pair, ...pairs.filter((item) => item.release.payload.version !== value.version)]
      .sort((a, b) => compareVersions(b.release.payload.version, a.release.payload.version)).slice(0, HISTORY_LIMIT);
    persistence = persistence.catch(() => {}).then(persist);
    await persistence;
    return noteView(value);
  }
  async function fetchFor(release, signal) {
    signal = AbortSignal.any([signal, AbortSignal.timeout(30000)].filter(Boolean));
    // Current metadata must satisfy the full installed transition floor, even if notes are absent.
    require('./release.cjs').verifyEnvelope(release, root, transitions);
    const response = await fetchAsset(releaseAssetUrl(release.payload.version, NOTES_ASSET), { fetchFn, signal });
    const notes = await readLimited(response, NOTES_LIMIT);
    signal?.throwIfAborted();
    return add(release, notes);
  }
  async function fetchVersion(version, signal) {
    signal = AbortSignal.any([signal, AbortSignal.timeout(30000)].filter(Boolean));
    const response = await fetchAsset(releaseAssetUrl(version, 'release-v3.json'), { fetchFn, signal });
    const release = await readLimited(response, 32 * 1024);
    if (release?.payload?.version !== version) throw new Error('Release-note history version mismatch.');
    return fetchFor(release, signal);
  }
  return { load, snapshot, add, fetchFor, fetchVersion };
}

module.exports = { createNotesHistory, HISTORY_LIMIT, CACHE_LIMIT };
