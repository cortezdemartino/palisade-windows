'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const { validateReport, parseStack, DiagnosticError } = require('../src/diagnostics/schema.cjs');
const { createStore } = require('../src/diagnostics/store.cjs');
const { createDiagnostics, promptRecovery, HANG_AFTER_MS } = require('../src/diagnostics/monitor.cjs');
const { installDiagnosticsBridge, INFO, PENDING, ACK, CRUMB, RECOVERY } = require('../src/diagnostics/bridge.cjs');

// Simulated Electron objects only: these tests do not run Electron or Windows. Real crash
// behaviour on Windows needs the packaged app (see docs/DIAGNOSTICS.md, "Verification").

const contract = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'tc-diagnostics-v1.json'), 'utf8'));
const tempDir = () => fs.mkdtempSync(path.join(os.tmpdir(), 'palisade-diag-'));

test('contract vectors: the desktop validator accepts and refuses what the Testing Center does', () => {
  for (const vector of contract['x-vectors']) {
    // The desktop app stores summaries without an installation id; the signed-in page adds it.
    if (vector.name === 'missing installation id is refused') continue;
    const report = { ...vector.report, occurredAt: vector.report.occurredAt === '$NOW' ? '2026-11-28T11:59:30Z' : vector.report.occurredAt };
    if (vector.expect === 'valid') {
      assert.doesNotThrow(() => validateReport(report), vector.name);
    } else {
      assert.throws(() => validateReport(report), (error) => error instanceof DiagnosticError && error.code === vector.expect && (!vector.field || error.field === vector.field), vector.name);
    }
  }
});

test('stack frames keep file names only, never the Windows user path', () => {
  const frames = parseStack([
    'TypeError: secret text',
    '    at verifyPackage (C:\\Users\\alice\\AppData\\Local\\Programs\\Palisade\\resources\\app.asar\\src\\updates\\updater.cjs:212:17)',
    '    at node:internal/process/task_queues:95:5',
    '    at async Promise.all (index 0)',
  ].join('\n'));
  assert.deepEqual(frames, [
    { module: 'updater.cjs', inApp: true, line: 212, column: 17, function: 'verifyPackage' },
    { module: 'task_queues', inApp: false, line: 95, column: 5 },
  ]);
  assert.ok(!JSON.stringify(frames).includes('alice'));
});

function sample(kind = 'process-crash') {
  return { schemaVersion: 1, reportId: require('node:crypto').randomUUID(), kind, classification: 'confirmed', occurredAt: '2026-11-28T12:00:00Z',
    client: { platform: 'windows-electron', appVersion: '0.3.6', buildId: 'win-0.3.6', channel: 'stable', os: { family: 'windows', version: '10.0.26100' } },
    sessionId: require('node:crypto').randomUUID(), process: 'renderer', subsystem: 'shell', reason: 'crashed', fatal: true, frames: [] };
}

test('store: bounded, expiring, atomic, and deletes anything invalid', () => {
  const dir = tempDir();
  let now = Date.parse('2026-11-28T12:00:00Z');
  const store = createStore({ dir, now: () => now });
  for (let i = 0; i < 25; i += 1) { now += 1; store.add(sample()); }
  now += 1;
  assert.equal(store.list().length, 20);
  assert.equal(store.add(sample(), 'same'), store.list().at(-1).localId);
  assert.equal(store.add(sample(), 'same'), undefined, 'repeat within ten minutes is not stored again');
  fs.writeFileSync(path.join(dir, 'pending', '00000000-0000-4000-8000-000000000000.json'), '{not json');
  fs.writeFileSync(path.join(dir, 'pending', 'evil.txt'), 'x');
  assert.equal(store.list().length, 20);
  assert.ok(!fs.existsSync(path.join(dir, 'pending', 'evil.txt')));
  assert.throws(() => store.add({ ...sample(), message: 'secret' }), DiagnosticError);
  const ids = store.list().slice(0, 3).map(e => e.localId);
  assert.equal(store.remove([...ids, '../../outside']), 3);
  now += 15 * 86400000;
  assert.equal(store.list().length, 0, 'expired after 14 days');
  assert.ok(fs.readdirSync(path.join(dir, 'pending')).every(n => !n.endsWith('.tmp')));
});

test('store: unclean ends are detected, three in ten minutes form a crash loop, clean exits reset', () => {
  const dir = tempDir();
  let now = 1_000_000;
  const store = createStore({ dir, now: () => now });
  assert.equal(store.beginSession({ sessionId: 'a', version: '0.3.6' }), undefined);
  now += 1000;
  const second = store.beginSession({ sessionId: 'b', version: '0.3.6' });
  assert.equal(second.crashLoop, false);
  assert.equal(second.reachedWindow, false);
  now += 1000; store.beginSession({ sessionId: 'c', version: '0.3.6' });
  now += 1000;
  const loop = store.beginSession({ sessionId: 'd', version: '0.3.6' });
  assert.equal(loop.crashLoop, true);
  assert.equal(loop.loopCount, 3);
  store.updateSession({ cleanExit: true });
  now += 1000;
  assert.equal(store.beginSession({ sessionId: 'e', version: '0.3.6' }), undefined);
});

test('store: local minidumps are bounded and expire', () => {
  const dir = tempDir();
  const dumps = path.join(dir, 'Crashpad', 'reports');
  fs.mkdirSync(dumps, { recursive: true });
  const old = path.join(dumps, 'old.dmp');
  fs.writeFileSync(old, 'MDMP');
  const t = (Date.now() - 20 * 86400000) / 1000;
  fs.utimesSync(old, t, t);
  fs.writeFileSync(path.join(dumps, 'new.dmp'), 'MDMP');
  const store = createStore({ dir });
  assert.deepEqual(store.sweepDumps(path.join(dir, 'Crashpad')), { removed: 1, kept: 1 });
});

function fakeApp() {
  return Object.assign(new EventEmitter(), { isPackaged: true, getVersion: () => '0.3.6', getPath: () => path.join(os.tmpdir(), 'no-dumps') });
}

test('monitor: local-only crash reporting, renderer, GPU and hang detection, expected revocation ignored', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const dir = tempDir();
  let clock = Date.now();
  const store = createStore({ dir, now: () => clock });
  const app = fakeApp();
  const started = [];
  const diagnostics = createDiagnostics({ app, crashReporter: { start: options => started.push(options) }, store, now: () => clock });
  diagnostics.start();
  assert.deepEqual(started, [{ uploadToServer: false, compress: true }]);
  diagnostics.record('update', 'updater', 'update.check', 'start');
  diagnostics.pageBreadcrumb({ at: Date.now(), id: require('node:crypto').randomUUID(), category: 'sync', subsystem: 'sync', action: 'sync.online', outcome: 'success' });
  diagnostics.pageBreadcrumb({ at: Date.now(), id: 'x', category: 'sync', subsystem: 'sync', action: 'typed text here', outcome: 'success' });
  diagnostics.onRenderGone({}, {}, { reason: 'clean-exit' });
  assert.equal(store.list().length, 0);
  diagnostics.expectRendererExit();
  diagnostics.onRenderGone({}, {}, { reason: 'crashed' });
  assert.equal(store.list().length, 0, 'Assist revocation is not a crash');
  clock += 6000;
  diagnostics.onRenderGone({}, {}, { reason: 'oom', exitCode: 3 });
  clock += 1;
  diagnostics.onChildGone({}, { type: 'GPU', reason: 'crashed', exitCode: 1 });
  clock += 1;
  diagnostics.onChildGone({}, { type: 'Utility', serviceName: 'audio.mojom.AudioService', reason: 'killed' });
  clock += 1;
  const contents = new EventEmitter();
  diagnostics.watchContents(contents);
  contents.emit('unresponsive');
  t.mock.timers.tick(HANG_AFTER_MS + 1);
  const reports = store.list().map(e => e.report);
  assert.deepEqual(reports.map(r => [r.kind, r.process, r.subsystem]), [
    ['oom', 'renderer', 'shell'], ['process-crash', 'gpu', 'ui'], ['process-crash', 'utility', 'voice'], ['hang', 'renderer', 'ui']]);
  const crumbs = reports[0].breadcrumbs;
  assert.ok(crumbs.some(c => c.action === 'sync.online'));
  assert.ok(!JSON.stringify(crumbs).includes('typed text'));
  assert.equal(reports[0].client.platform, 'windows-electron');
  assert.ok(reports.every(r => !('installationId' in r)));
  // A responsive window within the limit is not a hang.
  contents.emit('unresponsive'); contents.emit('responsive');
  t.mock.timers.tick(HANG_AFTER_MS + 1);
  assert.equal(store.list().length, 4);
});

test('monitor: main-process exceptions are observed without changing Electron’s own handling', () => {
  const dir = tempDir();
  const store = createStore({ dir });
  const before = process.listenerCount('uncaughtExceptionMonitor');
  const handlers = [process.listenerCount('uncaughtException'), process.listenerCount('unhandledRejection')];
  const diagnostics = createDiagnostics({ app: fakeApp(), crashReporter: { start() {} }, store });
  diagnostics.start();
  assert.deepEqual([process.listenerCount('uncaughtException'), process.listenerCount('unhandledRejection')], handlers,
    'no uncaughtException or unhandledRejection handler is added');
  const error = new TypeError('contains the user’s message');
  process.emit('uncaughtExceptionMonitor', error, 'uncaughtException');
  const [entry] = store.list();
  assert.equal(entry.report.kind, 'startup-failure', 'before the window was ready');
  assert.equal(entry.report.exceptionType, 'TypeError');
  assert.ok(!JSON.stringify(entry.report).includes('message'));
  for (const listener of process.listeners('uncaughtExceptionMonitor').slice(before)) process.removeListener('uncaughtExceptionMonitor', listener);
  // The next start sees the recorded crash, so it does not add an inferred unclean exit.
  const next = createDiagnostics({ app: fakeApp(), crashReporter: { start() {} }, store: createStore({ dir }) });
  next.start();
  for (const listener of process.listeners('uncaughtExceptionMonitor').slice(before)) process.removeListener('uncaughtExceptionMonitor', listener);
  assert.equal(store.list().filter(e => e.report.kind === 'unclean-exit').length, 0);
});

test('monitor: an unclean previous end is inferred, not a crash, and recovery is a choice', async () => {
  const dir = tempDir();
  const first = createDiagnostics({ app: fakeApp(), crashReporter: { start() {} }, store: createStore({ dir }) });
  const before = process.listenerCount('uncaughtExceptionMonitor');
  first.start();
  first.markWindowReady();
  // No clean exit: the process "disappears".
  const store = createStore({ dir });
  const second = createDiagnostics({ app: fakeApp(), crashReporter: { start() {} }, store });
  second.start();
  for (const listener of process.listeners('uncaughtExceptionMonitor').slice(before - 0)) process.removeListener('uncaughtExceptionMonitor', listener);
  const [entry] = store.list();
  assert.equal(entry.report.kind, 'unclean-exit');
  assert.equal(entry.report.classification, 'inferred');
  const prompts = [];
  const dialog = { showMessageBox: async (_w, options) => { prompts.push(options); return { response: 1 }; } };
  assert.equal(await promptRecovery({ dialog, window: null, recovery: second.recovery() }), false);
  assert.match(prompts[0].detail, /only if you allow crash reports/);
  second.markCleanExit();
  const third = createDiagnostics({ app: fakeApp(), crashReporter: { start() {} }, store: createStore({ dir }) });
  third.start();
  for (const listener of process.listeners('uncaughtExceptionMonitor').slice(before - 0)) process.removeListener('uncaughtExceptionMonitor', listener);
  assert.equal(await promptRecovery({ dialog, window: null, recovery: third.recovery() }), false);
  assert.equal(prompts.length, 1, 'no prompt after a clean exit');
});

function fakeIpc() {
  const handlers = new Map(), listeners = new Map();
  return { handlers, listeners, handle: (c, f) => handlers.set(c, f), removeHandler: c => handlers.delete(c),
    on: (c, f) => listeners.set(c, f), removeListener: c => listeners.delete(c) };
}

test('bridge: only the trusted Palisade main frame, exact arguments, rate limited', async () => {
  const dir = tempDir();
  const store = createStore({ dir });
  store.add(sample());
  const diagnostics = createDiagnostics({ app: fakeApp(), crashReporter: { start() {} }, store });
  const sent = [];
  const mainFrame = { url: 'https://palisade.onyxops.org/' };
  const contents = Object.assign(new EventEmitter(), { mainFrame, isDestroyed: () => false, getURL: () => 'https://palisade.onyxops.org/', send: c => sent.push(c) });
  const ipc = fakeIpc();
  let now = 0;
  const bridge = installDiagnosticsBridge(contents, { diagnostics, store, ipc, now: () => now });
  const trusted = { sender: contents, senderFrame: mainFrame };
  const info = ipc.handlers.get(INFO)(trusted);
  assert.equal(info.protocol, 'palisade.diagnostics');
  assert.equal(info.recovery.requested, false);
  assert.throws(() => ipc.handlers.get(INFO)({ sender: contents, senderFrame: { url: 'https://evil.example/' } }));
  assert.throws(() => ipc.handlers.get(INFO)({ sender: {}, senderFrame: mainFrame }));
  assert.throws(() => ipc.handlers.get(INFO)(trusted, 'extra'));
  const pending = ipc.handlers.get(PENDING)(trusted);
  assert.equal(pending.length, 1);
  assert.deepEqual(Object.keys(pending[0]).sort(), ['category', 'localId', 'report']);
  assert.throws(() => ipc.handlers.get(ACK)(trusted, ['../x']));
  assert.throws(() => ipc.handlers.get(ACK)(trusted, 'not-a-list'));
  now += 1000;
  for (let i = 0; i < 10; i += 1) ipc.handlers.get(INFO)(trusted);
  assert.throws(() => ipc.handlers.get(ACK)(trusted, [pending[0].localId]), /refused/, 'rate limited');
  now += 1000;
  assert.deepEqual(ipc.handlers.get(ACK)(trusted, [pending[0].localId]), { removed: 1 });
  bridge.requestRecovery();
  assert.deepEqual(sent, [RECOVERY]);
  assert.equal(ipc.handlers.get(INFO)(trusted).recovery.requested, true);
  assert.equal(ipc.handlers.get(INFO)(trusted).recovery.requested, false, 'consumed once');
  ipc.listeners.get(CRUMB)(trusted, { at: Date.now(), id: require('node:crypto').randomUUID(), category: 'voice', subsystem: 'voice', action: 'voice.join', outcome: 'failure' });
  assert.ok(diagnostics.crumbs().some(c => c.action === 'voice.join'));
  contents.emit('destroyed');
  assert.equal(ipc.handlers.size, 0);
});
