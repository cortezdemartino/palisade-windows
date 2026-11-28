'use strict';
const crypto = require('node:crypto');
const os = require('node:os');
const { validateBreadcrumb, parseStack, exceptionType, CATEGORIES, SUBSYSTEMS, OUTCOMES, ACTION, ERROR_CODE } = require('./schema.cjs');

// Desktop crash and failure detection (project-palisade docs/68 §16, docs/69 Electron).
//
// Detects, with Electron's supported APIs only: renderer termination (render-process-gone),
// GPU/utility/other child process termination (child-process-gone), an unresponsive window
// (unresponsive/responsive), main-process exceptions (uncaughtExceptionMonitor: it observes,
// it does not catch, so Electron's own handling of an unrecoverable main-process error is
// unchanged), unclean previous exits and crash loops (session marker), updater failures and
// native Assist helper failures (status reported by their owners). Native crashes are written
// by crashReporter as local minidumps that are never uploaded (uploadToServer: false).
//
// Every summary is validated against the Testing Center contract before it is stored, and is
// only stored: the signed-in page sends it later, with consent (bridge.cjs). A diagnostics fault
// never stops Palisade: every hook is wrapped and failures are ignored.

const HANG_AFTER_MS = 15000;
const MAX_CRUMBS = 50;
const CRUMB_AGE_MS = 30 * 60000;

function createDiagnostics({ app, crashReporter, store, now = Date.now, packaged = app.isPackaged, buildId }) {
  const sessionId = crypto.randomUUID();
  const startedAt = now();
  const mainCrumbs = [];
  const pageCrumbs = [];
  let expectRendererExitUntil = 0;
  let windowReady = false;
  let previous;
  let identityCache;

  const safe = fn => (...args) => { try { return fn(...args); } catch { return undefined; } };

  function identity() {
    if (identityCache) return identityCache;
    const version = app.getVersion();
    const release = /^[0-9]{1,5}(\.[0-9]{1,6}){0,3}$/.test(os.release()) ? os.release() : null;
    identityCache = {
      platform: 'windows-electron',
      appVersion: version,
      buildId: buildId || ('win-' + version),
      channel: packaged ? 'stable' : 'internal',
      os: process.platform === 'win32' ? { family: 'windows', version: release } : { family: process.platform === 'darwin' ? 'macos' : process.platform === 'linux' ? 'linux' : 'other', version: null },
    };
    return identityCache;
  }

  function push(list, crumb) {
    list.push(crumb);
    while (list.length > MAX_CRUMBS) list.shift();
  }
  const record = safe((category, subsystem, action, outcome, code) => {
    if (!CATEGORIES.includes(category) || !SUBSYSTEMS.includes(subsystem) || !OUTCOMES.includes(outcome) || !ACTION.test(action)) return;
    const crumb = { at: now(), id: crypto.randomUUID(), category, subsystem, action, outcome };
    if (code !== undefined) { if (!ERROR_CODE.test(code)) return; crumb.code = code; }
    push(mainCrumbs, crumb);
  });
  /** Breadcrumbs the page forwards (already sanitized there; checked again here). */
  const pageBreadcrumb = safe(value => { push(pageCrumbs, validateBreadcrumb(value)); });
  const crumbs = () => [...mainCrumbs, ...pageCrumbs].filter(c => now() - c.at < CRUMB_AGE_MS).sort((a, b) => a.at - b.at).slice(-MAX_CRUMBS);

  function summary(fields) {
    const report = {
      schemaVersion: 1, reportId: crypto.randomUUID(), classification: 'confirmed',
      occurredAt: new Date(now()).toISOString().replace(/\.\d{3}Z$/, 'Z'), client: identity(), sessionId,
      frames: [], fatal: false, uptimeMs: Math.min(31 * 86400000, Math.max(0, now() - startedAt)), ...fields,
    };
    const list = crumbs();
    if (list.length) report.breadcrumbs = list;
    try {
      const mb = Math.round(process.memoryUsage().rss / 1048576);
      if (Number.isInteger(mb)) report.memoryMb = mb;
    } catch {}
    return report;
  }
  const store_ = safe((fields, dedupeKey) => store.add(summary(fields), dedupeKey));

  /** Call before app 'ready'. Starts local-only crash dumps and reads the previous session. */
  const start = safe(() => {
    try { crashReporter.start({ uploadToServer: false, compress: true }); } catch {}
    previous = store.beginSession({ sessionId, version: app.getVersion() });
    record('init', 'startup', 'app.start', 'start');
    if (previous && !previous.crashRecorded) {
      // Nothing confirmed a crash: an unclean end is inferred, and may be ordinary (power, updates).
      // Before the window appeared it may have been a failed start, but that is not confirmed.
      const kind = previous.crashLoop ? 'crash-loop' : 'unclean-exit';
      // correlationId: the session that ended uncleanly, so stability counts it for that session.
      store_({ kind, classification: 'inferred', process: 'main', subsystem: previous.reachedWindow ? 'shell' : 'startup',
        ...(/^[0-9a-f-]{36}$/.test(previous.sessionId || '') ? { correlationId: previous.sessionId } : {}),
        reason: 'unknown', fatal: true, crashLoopCount: Math.min(100, previous.loopCount) });
    } else if (previous?.crashLoop) {
      store_({ kind: 'crash-loop', classification: 'inferred', process: 'main', subsystem: 'startup', reason: 'crashed', fatal: true, crashLoopCount: Math.min(100, previous.loopCount) });
    }
    try { store.sweepDumps(app.getPath('crashDumps')); } catch {}
    process.on('uncaughtExceptionMonitor', (error, origin) => {
      // Synchronous on purpose: the process may end right after this returns.
      const kind = !windowReady ? 'startup-failure' : origin === 'unhandledRejection' ? 'unhandled-rejection' : 'exception';
      store_({ kind, process: 'main', subsystem: windowReady ? 'shell' : 'startup',
        fatal: true, frames: parseStack(error && error.stack), ...(exceptionType(error) ? { exceptionType: exceptionType(error) } : {}) });
      try { store.updateSession({ crashRecorded: true }); } catch {}
    });
  });

  /** Assist revocation ends the renderer on purpose (capture.cjs); that is not a defect. */
  const expectRendererExit = () => { expectRendererExitUntil = now() + 5000; };

  const onRenderGone = safe((_event, _contents, details) => {
    const reason = details && details.reason;
    record('process', 'shell', 'renderer.gone', 'failure');
    if (reason === 'clean-exit' || now() < expectRendererExitUntil) return;
    const mapped = ['crashed', 'oom', 'killed', 'abnormal-exit', 'launch-failed', 'integrity-failure', 'memory-eviction'].includes(reason) ? reason : 'unknown';
    const kind = reason === 'oom' ? 'oom' : reason === 'launch-failed' ? 'startup-failure' : 'process-crash';
    store_({ kind, process: 'renderer', subsystem: 'shell', reason: mapped, fatal: true }, 'renderer:' + mapped);
    try { store.updateSession({ crashRecorded: true }); } catch {}
  });

  const onChildGone = safe((_event, details) => {
    if (!details || details.reason === 'clean-exit') return;
    const type = String(details.type || '');
    const service = String(details.serviceName || details.name || '');
    const process_ = type === 'GPU' ? 'gpu' : 'utility';
    const subsystem = /audio|video|media/i.test(service) ? 'voice' : /network/i.test(service) ? 'network' : type === 'GPU' ? 'ui' : 'native';
    const mapped = ['crashed', 'oom', 'killed', 'abnormal-exit', 'launch-failed', 'integrity-failure', 'memory-eviction'].includes(details.reason) ? details.reason : 'unknown';
    record('process', subsystem, 'child.gone', 'failure');
    store_({ kind: details.reason === 'oom' ? 'oom' : 'process-crash', process: process_, subsystem, reason: mapped, fatal: false }, 'child:' + type + ':' + service + ':' + mapped);
  });

  /** Watches one window's page for freezes: unresponsive for 15 s is reported as a hang. */
  const watchContents = safe(contents => {
    let timer = null;
    contents.on('unresponsive', () => {
      record('process', 'ui', 'window.unresponsive', 'warning');
      if (timer) return;
      timer = setTimeout(() => {
        timer = null;
        store_({ kind: 'hang', process: 'renderer', subsystem: 'ui', reason: 'unresponsive', fatal: false }, 'hang:renderer');
      }, HANG_AFTER_MS);
      timer.unref?.();
    });
    contents.on('responsive', () => {
      record('process', 'ui', 'window.responsive', 'info');
      if (timer) { clearTimeout(timer); timer = null; }
    });
  });

  const failure = safe((kind, process_, subsystem, code) => {
    record('error', subsystem, kind === 'update-failure' ? 'update.failed' : 'helper.failed', 'failure', code);
    store_({ kind, process: process_, subsystem, errorCode: code, fatal: false }, kind + ':' + code);
  });

  const markWindowReady = safe(() => { windowReady = true; store.updateSession({ reachedWindow: true }); record('init', 'shell', 'window.ready', 'success'); });
  const markCleanExit = safe(() => { store.updateSession({ cleanExit: true }); });

  /** What the previous run left behind, for the recovery prompt. */
  const recovery = () => ({ previous, pending: safe(() => store.list().length)() || 0 });

  return { sessionId, start, record, pageBreadcrumb, onRenderGone, onChildGone, watchContents, expectRendererExit,
    updateFailed: code => failure('update-failure', 'updater', 'updater', code),
    helperFailed: code => failure('helper-failure', 'native-helper', 'assist', code),
    markWindowReady, markCleanExit, recovery, identity, crumbs };
}

/** The recovery prompt after an unexpected end. Reporting is always the person's choice. */
async function promptRecovery({ dialog, window, recovery }) {
  const { previous, pending } = recovery;
  if (!previous && pending === 0) return false;
  if (!previous) return false;
  const loop = previous.crashLoop;
  const result = await dialog.showMessageBox(window, {
    type: 'warning', title: 'Palisade closed unexpectedly',
    message: loop ? 'Palisade has closed unexpectedly several times in a few minutes.' : 'Palisade did not close normally last time.',
    detail: (loop
      ? 'If it keeps happening, check for updates in Settings → Updates, or reinstall with the official installer. '
      : 'This can also happen when Windows restarts or the PC loses power. ') +
      'You can tell the team what happened. A short crash summary (no messages, files or memory contents) is sent only if you allow crash reports.',
    buttons: ['Report the problem', 'Not now'], defaultId: 0, cancelId: 1, noLink: true,
  });
  return result.response === 0;
}

module.exports = { createDiagnostics, promptRecovery, HANG_AFTER_MS };
