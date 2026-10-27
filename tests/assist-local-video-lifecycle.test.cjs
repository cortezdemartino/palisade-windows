'use strict';
const test = require('node:test'), assert = require('node:assert/strict'), vm = require('node:vm'), fs = require('node:fs');
const { EventEmitter } = require('node:events');
const turn = () => new Promise(resolve => setImmediate(resolve));
function deferred() { let reject; const promise = new Promise((_resolve, fail) => { reject = fail; }); return { promise, reject }; }
test('old provider load rejection, crash, load failure and queued timeout cannot stop a successor', async () => {
  const ipc = new EventEmitter(), providers = [], timers = [];
  class Provider extends EventEmitter {
    constructor() {
      super(); this.destroyed = false; this.load = deferred(); providers.push(this);
      const noop = () => {};
      this.webContents = Object.assign(new EventEmitter(), { mainFrame: { url: 'palisade-controls://app/assist-provider.html' },
        isDestroyed: () => this.destroyed, getURL: () => 'palisade-controls://app/assist-provider.html', send: noop, setWindowOpenHandler: noop,
        session: { webRequest: { onBeforeRequest: noop }, setPermissionRequestHandler: noop, setPermissionCheckHandler: noop, setDisplayMediaRequestHandler: noop, setDevicePermissionHandler: noop } });
    }
    isDestroyed() { return this.destroyed; }
    destroy() { this.destroyed = true; this.emit('closed'); }
    loadURL() { return this.load.promise; }
  }
  const consumer = Object.assign(new EventEmitter(), { mainFrame: { url: 'https://palisade.onyxops.org/' }, getURL: () => 'https://palisade.onyxops.org/', isDestroyed: () => false, send() {} });
  const parent = Object.assign(new EventEmitter(), { isDestroyed: () => false, isVisible: () => true, isMinimized: () => false });
  const context = { module: { exports: {} }, __dirname: require('node:path').dirname(require.resolve('../src/assist/local-video-hop.cjs')), AbortController, setTimeout: callback => { const timer = { callback, unref() {} }; timers.push(timer); return timer; }, clearTimeout() {}, setInterval: () => ({ unref() {} }), clearInterval() {},
    require: name => name === 'electron' ? { BrowserWindow: Provider, ipcMain: ipc } : name === '../desktop-window.cjs' ? { installControlsProtocol() {} } : name.startsWith('./') ? require('../src/assist/' + name.slice(2)) : name === '../security-policy.cjs' ? require('../src/security-policy.cjs') : require(name) };
  vm.runInNewContext(fs.readFileSync(require.resolve('../src/assist/local-video-hop.cjs'), 'utf8'), context);
  const hop = context.module.exports.createLocalVideoHop({ consumer, parent, enabled: true, inspect: async () => {} });
  ipc.emit('palisade:local-video-event', { sender: consumer, senderFrame: consumer.mainFrame }, { type: 'ready', document: '12345678-1234-1234-1234-123456789abc', operation: null, role: 'consumer' });
  await turn();
  const first = assert.rejects(hop.start({ kind: 'synthetic' }), /ended/); await turn();
  const oldTimer = timers.at(-1); hop.stop(); await first;
  const second = assert.rejects(hop.start({ kind: 'synthetic' }), /ended/); await turn();
  assert.equal(providers.length, 2); assert(hop.busy());
  providers[0].load.reject(Error('Late old load')); await turn();
  providers[0].webContents.emit('render-process-gone'); providers[0].webContents.emit('did-fail-load'); oldTimer.callback();
  assert(hop.busy()); assert.equal(providers[1].destroyed, false);
  hop.dispose(); await second; assert.equal(providers[1].destroyed, true);
});
