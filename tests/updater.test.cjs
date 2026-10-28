'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs/promises');
const path = require('node:path');
const { EventEmitter } = require('node:events');
async function harness({ response = 0, fail = false, release = null, networkFail = false, pending = false, packaged = true, realTransport, publicKey, beforeManager = async () => {}, copyWait } = {}) {
  const calls = [], prompts = [], statuses = [];
  let quit = 0, clock = 0, networkCalls = 0;
  const timers = [], intervals = [], offers = [], signals = [];
  const powerMonitor = new EventEmitter();
  const settle = () => new Promise(resolve => setImmediate(resolve));
  const app = Object.assign(new EventEmitter(), { isPackaged: packaged, getVersion: () => '0.2.3', getPath: () => 'C:\\Temp', quit: () => quit++ });
  const fakeFs = { constants: { COPYFILE_EXCL: 1 } };
  const promises = {
    readFile: async file => file.endsWith('update-public-key.pem') ? (publicKey || 'test-public-key') : '[]',
    mkdtemp: async prefix => { calls.push(['mkdir', prefix]); return 'C:\\Temp\\manager-123'; },
    copyFile: async (...args) => { calls.push(['copy', ...args]); if (copyWait) await copyWait; if (fail) throw new Error('missing manager'); },
    rm: async (...args) => { calls.push(['cleanup', ...args]); }
  };
  const timer = (list, fn, delay) => { const entry = { fn, delay, active: true, unref() {} }; list.push(entry); return entry; };
  const context = { __dirname: 'C:\\Apps\\Palisade\\resources\\app.asar\\src\\updates', AbortController, AbortSignal,
    setTimeout: (fn, delay) => timer(timers, fn, delay), setInterval: (fn, delay) => timer(intervals, fn, delay),
    clearTimeout: entry => { if(entry) entry.active = false; }, clearInterval: entry => { if(entry) entry.active = false; }, module: { exports: {} }, process: { platform: 'win32', resourcesPath: 'C:\\Apps\\Palisade\\resources', execPath: 'C:\\Apps\\Palisade\\Palisade.exe' },
    require: name => {
      if (name === 'electron') return { app, powerMonitor, dialog: { showMessageBox: async (...args) => { prompts.push(args.at(-1)); return { response }; } } };
      if (name === 'node:fs/promises') return promises;
      if (name === 'node:fs') return fakeFs;
      if (name === 'node:path') return path.win32;
      if (name === 'node:child_process') return { spawn: (...args) => {
        calls.push(['spawn', ...args]); const child = new EventEmitter(); child.unref = () => calls.push(['unref']);
        process.nextTick(() => child.emit('spawn')); return child;
      } };
      if (name === './release.cjs' && realTransport) return require('../src/updates/release.cjs');
      if (name === './release.cjs') return { fetchRelease: async (fetchFn, key, version, history) => {
        assert.equal(key, 'test-public-key'); assert.equal(version, '0.2.3'); assert.equal(history.length, 0);
        await fetchFn('https://api.github.com/releases', { signal: AbortSignal.timeout(10000) });
        return release;
      } };
      throw new Error('Unexpected dependency ' + name);
    } };
  vm.runInNewContext(await fs.readFile(path.join(__dirname, '../src/updates/updater.cjs'), 'utf8'), context);
  const controller = context.module.exports.createUpdateController(() => null, value => statuses.push(value), { now: () => clock,
    beforeManager,
    onAvailable: (manifest, openManager) => offers.push({ manifest, openManager }),
    fetchFn: realTransport || (async (_url, { signal }) => {
      networkCalls++; signals.push(signal);
      if (networkFail) throw new Error('offline');
      if (pending) await new Promise((_resolve, reject) => signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true }));
      return {};
    }),
  });
  return { controller, calls, prompts, statuses, quit: () => quit, offers, timers, intervals, app, powerMonitor, signals,
    networkCalls: () => networkCalls, setClock: value => { clock = value; }, settle,
    runTimer: async delay => { const timer = timers.find(entry => entry.active && entry.delay === delay); assert.ok(timer); timer.active = false; timer.fn(); await settle(); },
  };
}
test('client delegates updates to a copied manager outside its installation and exits after launch', async () => {
  const h = await harness(); await h.controller.openManager();
  const copy = h.calls.find(x => x[0] === 'copy');
  assert.equal(copy[1], 'C:\\Apps\\Palisade\\resources\\manager\\Palisade-Manager.exe');
  assert.equal(copy[2], 'C:\\Temp\\manager-123\\Palisade-Manager.exe');
  const launch = h.calls.find(x => x[0] === 'spawn');
  assert.equal(launch[1], copy[2]);
  assert.deepEqual(Array.from(launch[2]), ['--installation', 'C:\\Apps\\Palisade']);
  assert.equal(launch[3].shell, false); assert.equal(h.quit(), 1);
});
test('cancel keeps the client running and creates no installer or manager files', async () => {
  let preparations = 0;
  const h = await harness({ response: 1, beforeManager: async () => { preparations++; } }); await h.controller.openManager();
  assert.equal(h.quit(), 0); assert.equal(h.calls.length, 0);
  assert.equal(preparations, 0);
});

test('manager waits for the shared real cleanup barrier before copying, spawning or quitting', async () => {
  const { createAssistLifecycle } = require('../src/assist/lifecycle.cjs');
  const life = createAssistLifecycle(); let release, revoked = false;
  const cleanup = new Promise(resolve => { release = resolve; });
  life.hold({ revoke() { revoked = true; }, wait: () => cleanup });
  const h = await harness({ beforeManager: reason => life.quiesce(reason) });
  const opening = h.controller.openManager(); await h.settle();
  assert.equal(revoked, true); assert.equal(life.accepting(), false);
  assert.equal(h.calls.length, 0); assert.equal(h.quit(), 0);
  await h.controller.openManager(); assert.equal(h.prompts.length, 1);
  release(); await opening;
  assert.equal(life.status().state, 'closed'); assert.equal(h.calls.filter(row => row[0] === 'spawn').length, 1); assert.equal(h.quit(), 1);
});

test('failed or invalid cleanup preparation postpones updates without a manager or bypass instruction', async () => {
  for (const beforeManager of [async () => { throw Error('cleanup held'); }, () => false, async () => false]) {
    const h = await harness({ beforeManager }); await h.controller.openManager();
    assert.equal(h.calls.length, 0); assert.equal(h.quit(), 0);
    assert.match(h.statuses.at(-1), /postponed/);
    assert.match(h.prompts.at(-1).detail, /manager was not opened/);
    assert.doesNotMatch(h.prompts.at(-1).detail, /repair|official.*manager/i);
  }
});

test('missing exit preparation hook fails construction', async () => {
  await assert.rejects(harness({ beforeManager: null }), /Exit preparation hook required/);
});

test('shutdown during cleanup cancels manager opening before any files are created', async () => {
  let release; const preparation = new Promise(resolve => { release = resolve; });
  const h = await harness({ beforeManager: () => preparation });
  const opening = h.controller.openManager(); await h.settle();
  h.controller.stop(); release(); await opening;
  assert.equal(h.calls.length, 0); assert.equal(h.quit(), 0);
});

test('shutdown during manager copying removes only its temporary directory and never spawns', async () => {
  let release; const copyWait = new Promise(resolve => { release = resolve; });
  const h = await harness({ copyWait });
  const opening = h.controller.openManager(); await h.settle();
  assert(h.calls.some(row => row[0] === 'copy'));
  h.controller.stop(); release(); await opening;
  assert.equal(h.calls.some(row => row[0] === 'spawn'), false); assert.equal(h.quit(), 0);
  assert.equal(h.calls.at(-1)[0], 'cleanup');
  assert.equal(h.calls.at(-1)[1], 'C:\\Temp\\manager-123');
});
test('missing manager keeps client running and reports recovery without executing setup', async () => {
  const h = await harness({ fail: true }); await h.controller.openManager();
  assert.equal(h.quit(), 0); assert.equal(h.calls.some(x => x[0] === 'spawn'), false);
  assert.equal(h.prompts.at(-1).type, 'warning'); assert.equal(h.calls.at(-1)[0], 'cleanup');
});
test('startup checks immediately and polls every six hours without duplicate schedules', async () => {
  const h = await harness(); h.controller.start(); h.controller.start();
  assert.equal(h.networkCalls(), 0); assert.equal(h.intervals.length, 1);
  assert.equal(h.intervals[0].delay, 21600000);
  assert.equal(h.timers.length, 0);
  await h.settle(); assert.equal(h.networkCalls(), 1);
  h.intervals[0].fn(); await h.settle(); assert.equal(h.networkCalls(), 2);
  assert.equal(h.prompts.length, 0); assert.equal(h.calls.length, 0);
  assert.match(h.statuses.at(-1), /up to date/); h.controller.stop();
});

test('verified offers notify once per version without launching or downloading an installer', async () => {
  const h = await harness({ release: { version: '0.2.4' } });
  await h.controller.check(); await h.controller.check();
  assert.equal(h.offers.length, 1); assert.equal(h.prompts.length, 0); assert.equal(h.calls.length, 0);
  assert.match(h.statuses.at(-1), /0.2.4 available/);
  h.offers[0].openManager(); await h.settle(); assert.equal(h.quit(), 1);
});

test('network or verification failures retry with backoff and never announce an update', async () => {
  const h = await harness({ networkFail: true }); h.controller.start();
  await h.settle(); assert.equal(h.offers.length, 0); assert.equal(h.prompts.length, 0);
  await h.runTimer(900000); assert.equal(h.networkCalls(), 2);
  assert.ok(h.timers.some(timer => timer.active && timer.delay === 1800000));
  assert.match(h.statuses.at(-1), /failed/); assert.equal(h.calls.length, 0); h.controller.stop();
});

test('resume checks only when overdue and stops scheduling on shutdown', async () => {
  const h = await harness(); h.controller.start(); await h.settle();
  h.setClock(1000); h.powerMonitor.emit('resume'); await h.settle(); assert.equal(h.networkCalls(), 1);
  h.setClock(21600001); h.powerMonitor.emit('resume'); await h.settle(); assert.equal(h.networkCalls(), 2);
  h.app.emit('before-quit'); assert.equal(h.powerMonitor.listenerCount('resume'), 1);
  h.app.emit('will-quit'); assert.equal(h.powerMonitor.listenerCount('resume'), 0);
  assert.equal(h.timers.some(timer => timer.active), false); assert.equal(h.intervals[0].active, false);
  await h.controller.check(); assert.equal(h.networkCalls(), 2);
});

test('concurrent checks are coalesced and shutdown aborts an in-flight request', async () => {
  const h = await harness({ pending: true });
  h.controller.start(); await h.settle(); await h.controller.check();
  assert.equal(h.networkCalls(), 1); h.controller.stop(); await h.settle();
  assert.equal(h.signals[0].aborted, true); assert.equal(h.offers.length, 0);
  assert.equal(h.timers.filter(timer => timer.active).length, 0);
});

test('manual check offers release notes and Later never opens the manager', async () => {
  const h = await harness({ response: 1, release: { version: '0.2.4', releaseNotes: 'Release changes' } });
  await h.controller.check(true);
  assert.equal(h.prompts[0].detail, 'Release changes'); assert.equal(h.calls.length, 0);
  assert.equal(h.quit(), 0);
});

test('development clients do not start network polling', async () => {
  const h = await harness({ packaged: false }); h.controller.start(); await h.controller.check();
  assert.equal(h.networkCalls(), 0); assert.equal(h.timers.length, 0); assert.equal(h.intervals.length, 0);
});

test('polling announces only a genuinely signed newer stable release and rejects tampering and prereleases', async () => {
  const { generateKeyPairSync, createHash, sign } = require('node:crypto');
  const releases = require('../src/updates/release.cjs');
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  const key = publicKey.export({ type: 'spki', format: 'pem' });
  const bytes = Buffer.from('installer fixture');
  const payload = releases.normalisePayload({ schema: 3, channel: 'stable', version: '0.2.4', filename: 'Palisade-Setup-0.2.4-x64.exe', size: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex'), sha512: createHash('sha512').update(bytes).digest('base64'),
    downloadUrl: releases.releaseAssetUrl('0.2.4', 'Palisade-Setup-0.2.4-x64.exe'), publishedAt: '2026-10-17T12:00:00.000Z' });
  const signature = sign(null, releases.signedBytes(payload), privateKey).toString('base64');
  for (const mode of ['valid', 'tampered', 'prerelease', 'downgrade']) {
    const transport = async url => {
      const metadata = url === releases.RELEASES_API;
      const version = mode === 'downgrade' ? '0.2.2' : '0.2.4';
      const info = { draft: false, prerelease: mode === 'prerelease', tag_name: 'v' + version, body: 'Release notes', assets: [{ name: 'release.json' }, { name: 'Palisade-Setup-' + version + '-x64.exe' }] };
      const envelope = { payload: mode === 'tampered' ? { ...payload, sha256: '0'.repeat(64) } : payload, signature, transitions: [] };
      const response = new Response(JSON.stringify(metadata ? info : envelope), { headers: { 'Content-Type': 'application/json' } });
      Object.defineProperty(response, 'url', { value: url }); return response;
    };
    const h = await harness({ realTransport: transport, publicKey: key }); await h.controller.check();
    assert.equal(h.offers.length, mode === 'valid' ? 1 : 0, mode);
    if (mode === 'tampered' || mode === 'prerelease') assert.match(h.statuses.at(-1), /failed/);
    assert.equal(h.calls.length, 0);
  }
});
