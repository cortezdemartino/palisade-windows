'use strict';
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

// Desktop telemetry (project-palisade docs/68 §24, docs/69 Electron): lifecycle, update and
// performance events of the Windows app, in the Testing Center event envelope. Like crash
// summaries (store.cjs), they are only stored on the PC; the signed-in page sends them with the
// person's consent (usage analytics for lifecycle and updates, performance for measurements)
// and acknowledges them. Bounded: 200 events, 128 KiB, 7 days. Sampling of resource
// measurements: every 10 minutes while the app runs, so overhead stays negligible.

const MAX_EVENTS = 200;
const MAX_BYTES = 128 * 1024;
const MAX_AGE_MS = 7 * 86400000;
const SAMPLE_EVERY_MS = 10 * 60000;
const ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const PHASES = ['launch', 'ready', 'foreground', 'background', 'idle', 'active', 'quit', 'recover', 'migrate'];
const METRICS = ['cold-start', 'warm-start', 'time-to-interactive', 'memory-mb', 'memory-peak-mb', 'cpu-percent', 'process-count', 'event-loop-lag', 'ipc-latency', 'queue-size', 'disk-mb'];

function createTelemetry({ dir, identity, sessionId, now = Date.now, files = fs, app = null }) {
  const eventsDir = path.join(dir, 'events');
  files.mkdirSync(eventsDir, { recursive: true });
  const versionFile = path.join(dir, 'last-version.json');
  let timers = [];
  let peakMb = 0;
  let seq = 0;

  function list() {
    let names = [];
    try { names = files.readdirSync(eventsDir); } catch { return []; }
    const out = [];
    for (const name of names) {
      const id = name.replace(/\.json$/, '');
      const file = path.join(eventsDir, name);
      if (!ID.test(id)) { try { files.rmSync(file, { force: true }); } catch {} continue; }
      try {
        const entry = JSON.parse(files.readFileSync(file, 'utf8'));
        if (now() - entry.storedAt > MAX_AGE_MS) throw Error('expired');
        out.push({ localId: id, event: entry.event, consent: entry.consent, storedAt: entry.storedAt, seq: entry.seq || 0 });
      } catch { try { files.rmSync(file, { force: true }); } catch {} }
    }
    return out.sort((a, b) => a.storedAt - b.storedAt || a.seq - b.seq);
  }

  function emit(type, payload, consent) {
    try {
      const event = { schemaVersion: 1, eventId: crypto.randomUUID(), type,
        occurredAt: new Date(now()).toISOString().replace(/\.\d{3}Z$/, 'Z'), client: identity(), sessionId,
        consentClass: consent, sampling: { rate: 1 }, payload };
      const localId = crypto.randomUUID();
      const file = path.join(eventsDir, localId + '.json');
      files.writeFileSync(file + '.tmp', JSON.stringify({ storedAt: now(), seq: ++seq, consent, event }), { mode: 0o600 });
      files.renameSync(file + '.tmp', file);
      let entries = list();
      let size = entries.reduce((a, e) => a + JSON.stringify(e.event).length, 0);
      while (entries.length > MAX_EVENTS || size > MAX_BYTES) {
        const oldest = entries.shift();
        if (!oldest) break;
        size -= JSON.stringify(oldest.event).length;
        try { files.rmSync(path.join(eventsDir, oldest.localId + '.json'), { force: true }); } catch {}
      }
      return localId;
    } catch { return undefined; }
  }

  const lifecycle = (phase, extra = {}) => PHASES.includes(phase) ? emit('app.lifecycle', { phase, ...extra }, 'analytics') : undefined;
  const measure = (metric, value, unit, activity, processType = 'main') => {
    if (!METRICS.includes(metric) || !Number.isFinite(value) || value < 0) return undefined;
    return emit('performance.measure', { metric, value: Math.round(value * 100) / 100, unit, process: processType, ...(activity ? { activity } : {}) }, 'performance');
  };
  const update = (phase, outcome, errorCode) => emit('update.lifecycle', { phase, outcome, ...(errorCode ? { errorCode } : {}) }, 'diagnostics');

  /** Launch: reports a version change (migration) and a recovery after an unclean end. */
  function onLaunch({ previous }) {
    lifecycle('launch', { coldStart: true });
    try {
      const last = JSON.parse(files.readFileSync(versionFile, 'utf8'));
      if (last.version !== identity().appVersion) lifecycle('migrate');
    } catch {}
    try { files.writeFileSync(versionFile, JSON.stringify({ version: identity().appVersion }), { mode: 0o600 }); } catch {}
    if (previous) lifecycle('recover', { previous: previous.crashRecorded ? 'crashed' : 'unclean' });
  }

  /** Resource sampling with supported APIs: app.getAppMetrics() and an event-loop timer. */
  function startSampling(isActive = () => false) {
    stopSampling();
    let expected = now() + 1000;
    const lag = setInterval(() => {
      const late = now() - expected;
      expected = now() + 1000;
      if (late > 200) measure('event-loop-lag', late, 'ms', isActive() ? 'active' : 'idle');
    }, 1000);
    lag.unref?.();
    const sample = setInterval(() => {
      try {
        const activity = isActive() ? 'active' : 'idle';
        const metrics = app?.getAppMetrics?.() || [];
        const kb = metrics.reduce((a, m) => a + (m.memory?.workingSetSize || 0), 0);
        const cpu = metrics.reduce((a, m) => a + (m.cpu?.percentCPUUsage || 0), 0);
        if (kb > 0) {
          const mb = kb / 1024;
          peakMb = Math.max(peakMb, mb);
          measure('memory-mb', mb, 'mb', activity, 'all');
          measure('memory-peak-mb', peakMb, 'mb', activity, 'all');
        }
        if (metrics.length) {
          measure('cpu-percent', cpu, 'percent', activity, 'all');
          measure('process-count', metrics.length, 'count', activity, 'all');
        }
        measure('queue-size', list().length, 'count', activity);
      } catch {}
    }, SAMPLE_EVERY_MS);
    sample.unref?.();
    timers = [lag, sample];
  }
  function stopSampling() { for (const t of timers) clearInterval(t); timers = []; }

  function remove(ids) {
    let removed = 0;
    for (const id of ids) {
      if (typeof id !== 'string' || !ID.test(id)) continue;
      try { files.rmSync(path.join(eventsDir, id + '.json'), { force: true }); removed += 1; } catch {}
    }
    return removed;
  }

  return { lifecycle, measure, update, onLaunch, startSampling, stopSampling, list, remove };
}

module.exports = { createTelemetry, METRICS, PHASES };
