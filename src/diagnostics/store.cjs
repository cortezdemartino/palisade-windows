'use strict';
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { validateReport, consentFor } = require('./schema.cjs');

// Pending crash and error summaries on this PC (userData/diagnostics). Bounded: at most 20
// summaries and 256 KiB, each kept at most 14 days. Writes are atomic (temp file + rename);
// anything unreadable or no longer valid is deleted rather than sent. Summaries leave the PC
// only through the signed-in Palisade page, with the person's consent (bridge.cjs), and are
// deleted here once it has queued them.
//
// The session marker records whether the previous run ended cleanly. An unclean end is only
// an *inferred* problem: Windows updates, power loss and Task Manager end processes too, so it
// is reported as `unclean-exit` with classification `inferred`, never as a crash. Three unclean
// ends within ten minutes form a crash loop.

const MAX_PENDING = 20;
const MAX_BYTES = 256 * 1024;
const MAX_AGE_MS = 14 * 86400000;
const LOOP_WINDOW_MS = 10 * 60000;
const LOOP_COUNT = 3;
const DEDUPE_MS = 10 * 60000;
const DUMP_MAX_AGE_MS = 14 * 86400000;
const DUMP_MAX_BYTES = 64 * 1024 * 1024;
const ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function createStore({ dir, now = Date.now, files = fs }) {
  const pendingDir = path.join(dir, 'pending');
  const markerFile = path.join(dir, 'session.json');
  const startsFile = path.join(dir, 'unclean-starts.json');
  const recent = new Map();
  files.mkdirSync(pendingDir, { recursive: true });

  function atomicWrite(file, text) {
    const temp = file + '.' + crypto.randomUUID() + '.tmp';
    files.writeFileSync(temp, text, { mode: 0o600 });
    files.renameSync(temp, file);
  }
  function readJson(file) {
    try { return JSON.parse(files.readFileSync(file, 'utf8')); } catch { return undefined; }
  }

  function list() {
    const out = [];
    let names = [];
    try { names = files.readdirSync(pendingDir); } catch { return out; }
    for (const name of names) {
      const file = path.join(pendingDir, name);
      const id = name.replace(/\.json$/, '');
      if (!name.endsWith('.json') || !ID.test(id)) { try { files.rmSync(file, { force: true }); } catch {} continue; }
      const entry = readJson(file);
      try {
        if (!entry || now() - entry.storedAt > MAX_AGE_MS) throw Error('expired');
        validateReport(entry.report);
        out.push({ localId: id, report: entry.report, category: consentFor(entry.report.kind), storedAt: entry.storedAt });
      } catch { try { files.rmSync(file, { force: true }); } catch {} }
    }
    return out.sort((a, b) => a.storedAt - b.storedAt);
  }

  /** Stores a validated summary; returns its local id, or undefined when it was a repeat. */
  function add(report, dedupeKey) {
    validateReport(report);
    if (dedupeKey) {
      const last = recent.get(dedupeKey);
      if (last !== undefined && now() - last < DEDUPE_MS) return undefined;
      recent.set(dedupeKey, now());
      if (recent.size > 200) recent.delete(recent.keys().next().value);
    }
    const localId = crypto.randomUUID();
    atomicWrite(path.join(pendingDir, localId + '.json'), JSON.stringify({ storedAt: now(), report }));
    // Oldest first out, so a burst cannot fill the disk.
    let entries = list();
    let size = entries.reduce((a, e) => a + JSON.stringify(e.report).length, 0);
    while (entries.length > MAX_PENDING || size > MAX_BYTES) {
      const oldest = entries.shift();
      if (!oldest) break;
      size -= JSON.stringify(oldest.report).length;
      try { files.rmSync(path.join(pendingDir, oldest.localId + '.json'), { force: true }); } catch {}
    }
    return localId;
  }

  function remove(ids) {
    let removed = 0;
    for (const id of ids) {
      if (typeof id !== 'string' || !ID.test(id)) continue;
      try { files.rmSync(path.join(pendingDir, id + '.json'), { force: true }); removed += 1; } catch {}
    }
    return removed;
  }

  function clear() { remove(list().map(e => e.localId)); }

  /**
   * Starts a session: returns what the previous run left behind. Call once, early.
   * `previous` is undefined on first run or after a clean exit.
   */
  function beginSession({ sessionId, version }) {
    const previous = readJson(markerFile);
    let unclean;
    if (previous && previous.cleanExit === false && typeof previous.startedAt === 'number') {
      const starts = (readJson(startsFile) || []).filter(t => typeof t === 'number' && now() - t < LOOP_WINDOW_MS);
      starts.push(now());
      atomicWrite(startsFile, JSON.stringify(starts.slice(-10)));
      unclean = { sessionId: previous.sessionId, version: previous.version, startedAt: previous.startedAt,
        reachedWindow: previous.reachedWindow === true, crashRecorded: previous.crashRecorded === true,
        loopCount: starts.length, crashLoop: starts.length >= LOOP_COUNT };
    } else {
      try { files.rmSync(startsFile, { force: true }); } catch {}
    }
    atomicWrite(markerFile, JSON.stringify({ sessionId, version, startedAt: now(), cleanExit: false, reachedWindow: false, crashRecorded: false }));
    return unclean;
  }
  function updateSession(patch) {
    const current = readJson(markerFile);
    if (current) atomicWrite(markerFile, JSON.stringify({ ...current, ...patch }));
  }

  /** Local minidumps (crashReporter, never uploaded): delete old ones and keep the total bounded. */
  function sweepDumps(dumpDir) {
    let entries = [];
    const walk = d => {
      let names = [];
      try { names = files.readdirSync(d, { withFileTypes: true }); } catch { return; }
      for (const n of names) {
        const p = path.join(d, n.name);
        if (n.isDirectory()) walk(p);
        else if (n.name.endsWith('.dmp')) { try { const s = files.statSync(p); entries.push({ p, size: s.size, at: s.mtimeMs }); } catch {} }
      }
    };
    walk(dumpDir);
    let removed = 0;
    entries.sort((a, b) => a.at - b.at);
    let total = entries.reduce((a, e) => a + e.size, 0);
    for (const e of entries) {
      if (now() - e.at > DUMP_MAX_AGE_MS || total > DUMP_MAX_BYTES) {
        try { files.rmSync(e.p, { force: true }); total -= e.size; removed += 1; } catch {}
      }
    }
    return { removed, kept: entries.length - removed };
  }

  return { add, list, remove, clear, beginSession, updateSession, sweepDumps, MAX_PENDING, MAX_AGE_MS };
}

module.exports = { createStore };
