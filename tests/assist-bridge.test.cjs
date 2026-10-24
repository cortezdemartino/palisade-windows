'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { EventEmitter } = require('node:events');
function harness() {
  const handlers = new Map(); let clock = 0;
  const contents = Object.assign(new EventEmitter(), { mainFrame: { url: 'https://palisade.onyxops.org/' },
    getURL: () => contents.mainFrame.url, isDestroyed: () => false, sent: [], send: (...args) => contents.sent.push(args) });
  const context = { module: { exports: {} }, Object, require: name => name === 'electron' ? {
    ipcMain: { handle: (channel, handler) => handlers.set(channel, handler), removeHandler: channel => handlers.delete(channel) },
  } : require('../src/security-policy.cjs') };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/assist/bridge.cjs'), 'utf8'), context);
  const bridge = context.module.exports.installAssistBridge(contents, { desktopVersion: '0.3.0', now: () => clock });
  const event = { sender: contents, senderFrame: contents.mainFrame };
  return { bridge, contents, event, handlers, clock: value => { clock = value; },
    invoke: (request = { versions: [1] }, caller = event) => handlers.get('palisade:assist-capabilities')(caller, request) };
}
test('v1 main handler validates provenance, shape, rate and cleanup without widening capabilities', () => {
  const h = harness(), report = h.invoke();
  assert.equal(report.capture, 'unavailable'); assert.equal(report.nativeEndpoint, 'absent');
  assert.equal(report.localRevocation, false); assert.equal(report.emergencyStop, false);
  assert.equal(h.invoke({ versions: [9] }).version, 1);
  for (const caller of [{ ...h.event, sender: {} }, { ...h.event, senderFrame: {} }]) assert.throws(() => h.invoke(undefined, caller));
  for (const request of [{}, { versions: [] }, { versions: [1], command: 'input' }, { versions: ['1'] }, { versions: Array(9).fill(1) }, { versions: [Infinity] }]) assert.throws(() => h.invoke(request));
  h.contents.mainFrame.url = 'https://evil.test/'; assert.throws(() => h.invoke());
  h.contents.mainFrame.url = 'https://palisade.onyxops.org/';
  for (let i = 0; i < 10; i++) h.invoke();
  assert.throws(() => h.invoke(), /rate/); h.clock(1000); h.invoke();
  h.contents.emit('did-start-navigation', {}, 'https://palisade.onyxops.org/', false, true);
  assert.throws(() => h.invoke()); h.contents.emit('did-finish-load'); h.invoke();
  h.bridge.changed(); assert.equal(h.contents.sent[0][0], 'palisade:assist-capabilities-changed');
  h.contents.emit('destroyed'); assert.equal(h.handlers.size, 0);
});
test('capability reports are accepted by the actual pinned web adapter', async () => {
  const { stripTypeScriptTypes } = require('node:module');
  const { createHash } = require('node:crypto');
  const source = fs.readFileSync(path.join(__dirname, 'fixtures/desktopAssistBridge.web.ts'), 'utf8').replace(/\r\n/g, '\n');
  assert.equal(createHash('sha256').update(source).digest('hex'), '3a7ba6e58726270977d812a05fdf4ac108d6ad8f61553e2d8e48851112ccb40e');
  const web = await import('data:text/javascript;base64,' + Buffer.from(stripTypeScriptTypes(source)).toString('base64'));
  const report = JSON.parse(JSON.stringify(harness().invoke()));
  assert.equal(web.parseCapabilityReport(report).mode, 'desktop');
  const adapter = web.createDesktopAssistAdapter({ palisadeDesktopAssist: {
    protocol: 'palisade.assist.bridge', getCapabilities: request => { assert.deepEqual(request, { protocol: 'palisade.assist.bridge', versions: [1] }); return report; },
  } });
  assert.equal((await adapter.refresh()).capture, 'unavailable'); adapter.dispose();
  assert.equal(web.parseCapabilityReport({ ...report, nativeEndpoint: 'control' }).mode, 'desktop-error');
});
test('isolated preload exposes only the three canonical members, bounds requests and removes event listeners', async () => {
  const ipc = new EventEmitter(), calls = []; let api, unload;
  const report = JSON.parse(JSON.stringify(harness().invoke()));
  ipc.invoke = async (...args) => { calls.push(args); return report; };
  const context = { Object, process: { isMainFrame: true }, location: { origin: 'https://palisade.onyxops.org' },
    window: { addEventListener: (name, fn) => { if (name === 'unload') unload = fn; } },
    setTimeout, clearTimeout, require: () => ({ ipcRenderer: ipc, contextBridge: { exposeInMainWorld: (name, value) => { assert.equal(name, 'palisadeDesktopAssist'); api = value; } } }),
  };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/chat-notifications-preload.cjs'), 'utf8'), context);
  assert.equal(Object.keys(api).sort().join(','), 'getCapabilities,onCapabilitiesChanged,protocol');
  await api.getCapabilities({ protocol: 'palisade.assist.bridge', versions: [1] });
  assert.equal(calls[0][0], 'palisade:assist-capabilities');
  await assert.rejects(api.getCapabilities({ protocol: 'bad', versions: [1] }));
  let seen; const off = api.onCapabilitiesChanged(value => { seen = value; });
  ipc.emit('palisade:assist-capabilities-changed', { secret: 'event' }, report); assert.equal(seen, report);
  off(); seen = null; ipc.emit('palisade:assist-capabilities-changed', {}, report); assert.equal(seen, null);
  unload(); assert.equal(ipc.listenerCount('palisade:assist-capabilities-changed'), 0);
  for (const change of [{ process: { isMainFrame: false } }, { location: { origin: 'https://evil.test' } }]) {
    api = undefined; vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/chat-notifications-preload.cjs'), 'utf8'), { ...context, ...change }); assert.equal(api, undefined);
  }
});
