'use strict';
// Fake windows/timers, actual main transport and liveness code. No OS effects.
const test = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), vm = require('node:vm'), { EventEmitter } = require('node:events');
const { Liveness } = require('../src/assist/local-video-liveness.cjs');
const policy = require('../src/assist/remote-input-policy.cjs');
const flush = () => new Promise(resolve => setImmediate(resolve));
const fp = 'sha-256 ' + Array(32).fill('AB').join(':');
const own = 'sha-256 ' + Array(32).fill('CD').join(':');
const url = 'palisade-controls://app/assist-input.html';
const doc = '12345678-1234-1234-1234-123456789abc';
function sdp(fingerprint, setup) {
  return ['v=0', 'o=- 123 2 IN IP4 127.0.0.1', 's=-', 't=0 0', 'a=group:BUNDLE 0',
    'm=application 9 UDP/DTLS/SCTP webrtc-datachannel', 'c=IN IP4 0.0.0.0',
    'a=ice-ufrag:abcd', 'a=ice-pwd:abcdefghijklmnopqrstuvwxyz', 'a=fingerprint:' + fingerprint,
    'a=setup:' + setup, 'a=mid:0', 'a=sctp-port:5000'].join('\r\n') + '\r\n';
}
const packet = JSON.stringify({ p: 'palisade.assist.control', v: 2, session: 'session_fixture_0001', kind: 'release-all' });
function deferred() {
  let resolve; const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
}
async function setup({ onObserved = () => true, onPacket = () => {}, onCheck, inputProfile } = {}) {
  const ipc = new EventEmitter(), intervals = new Set();
  let now = 0, window, nonce, revocations = 0;
  function message(value) {
    ipc.emit('palisade:owned-input-event', { sender: window.webContents, senderFrame: window.webContents.mainFrame },
      { ...value, nonce, document: doc });
  }
  class FakeWindow extends EventEmitter {
    constructor(options) {
      super(); window = this; this.dead = false;
      nonce = options.webPreferences.additionalArguments[0].split('=')[1];
      this.webContents = Object.assign(new EventEmitter(), {
        mainFrame: { url }, sent: [], getURL: () => url, isDestroyed: () => this.dead,
        setWindowOpenHandler() {},
        session: { setPermissionRequestHandler() {}, setPermissionCheckHandler() {}, setDevicePermissionHandler() {},
          setDisplayMediaRequestHandler() {}, webRequest: { onBeforeRequest() {} } },
        send(_channel, value) {
          this.sent.push(value);
          if (value.type === 'offer') message({ type: 'answer', sdp: sdp(own, 'active'), fingerprint: own });
        },
      });
    }
    isDestroyed() { return this.dead; }
    destroy() { this.dead = true; this.emit('closed'); }
    loadURL() { queueMicrotask(() => message({ type: 'ready' })); return Promise.resolve(); }
  }
  const context = { module: { exports: {} }, __dirname: 'fixture', AbortController,
    setTimeout: () => ({ unref() {} }), clearTimeout() {},
    setInterval: callback => { intervals.add(callback); return { unref() {}, callback }; },
    clearInterval: timer => intervals.delete(timer.callback),
    require: name => {
      if (name === 'electron') return { BrowserWindow: FakeWindow, ipcMain: ipc };
      if (name === '../desktop-window.cjs') return { installControlsProtocol() {} };
      if (name === './local-video-liveness.cjs') return { Liveness: class extends Liveness { constructor() { super(() => now); } } };
      if (name === './remote-input-policy.cjs') return policy;
      if (name === 'node:perf_hooks') return { performance: { now: () => now } };
      return require(name);
    },
  };
  vm.runInNewContext(fs.readFileSync(require.resolve('../src/assist/remote-input-peer.cjs'), 'utf8'), context);
  const peer = await context.module.exports.createOwnedInputPeer({ enabled: true, session: 'session_fixture_0001', helperTransport: fp,
    onObserved, onPacket, onCheck, inputProfile, onCandidate() {}, onClosed() { revocations++; } });
  await peer.offer(sdp(fp, 'actpass'));
  function pulse() {
    for (const tick of intervals) tick();
    const challenge = window.webContents.sent.findLast(value => value.type === 'heartbeat');
    message({ type: 'pulse', sequence: challenge.sequence, challenge: challenge.challenge, fingerprint: fp, channel: true });
  }
  pulse(); await flush();
  return { peer, message, pulse, setTime: value => { now = value; }, revocations: () => revocations,
    sent: () => window.webContents.sent, listeners: () => ipc.listenerCount('palisade:owned-input-event') };
}
test('expired peer proof refuses packets even when the watchdog has not run', async () => {
  let packets = 0;
  const t = await setup({ onPacket() { packets++; } });
  try {
    assert.equal(t.peer.verified(), true);
    t.setTime(2000); // No interval/watchdog callback is executed.
    assert.equal(t.peer.verified(), false);
    t.message({ type: 'packet', text: packet }); await flush();
    assert.equal(packets, 0); assert.equal(t.revocations(), 1); assert.equal(t.listeners(), 0);
  } finally { await t.peer.close(); }
});
test('queued stale gestures retain original ingress for native replay checks and ordered owned release', async () => {
  const blocked = deferred(), received = [];
  const t = await setup({ onPacket: async (text, _signal, stamp) => {
    received.push({ text, stamp }); if (received.length === 1) await blocked.promise;
  } });
  const down = JSON.stringify({ p: 'palisade.assist.control', v: 2, session: 'session_fixture_0001', kind: 'key', down: true });
  try {
    t.message({ type: 'packet', text: down }); await flush();
    t.message({ type: 'packet', text: down });
    t.message({ type: 'packet', text: packet });
    t.setTime(200); blocked.resolve(); await flush(); await flush();
    assert.deepEqual(received.map(value => value.text), [down, down, packet]);
    assert(received.every(value => /^[0-9]+$/.test(value.stamp)));
    assert(BigInt(received[1].stamp) >= BigInt(received[0].stamp));
    assert.equal(t.revocations(), 0);
  } finally { blocked.resolve(); await t.peer.close(); }
});
test('ordinary queue saturation preserves a release slot without overtaking earlier input', async () => {
  const blocked = deferred(), received = [];
  const t = await setup({ onPacket: async text => { received.push(text); if (received.length === 1) await blocked.promise; } });
  const down = JSON.stringify({ p: 'palisade.assist.control', v: 2, session: 'session_fixture_0001', kind: 'key', down: true });
  try {
    for (let n = 0; n < 8; n++) t.message({ type: 'packet', text: down });
    t.message({ type: 'packet', text: packet });
    await flush(); assert.equal(received.length, 1); assert.equal(t.revocations(), 0);
    blocked.resolve(); for (let n = 0; n < 15; n++) await flush();
    assert.equal(received.length, 9); assert.equal(received.at(-1), packet);
  } finally { blocked.resolve(); await t.peer.close(); }
});
test('native health refusal fences an otherwise healthy certificate and channel', async () => {
  let valid = true;
  const t = await setup({ onCheck: async () => { if (!valid) throw Error('revoked'); } });
  try {
    valid = false; t.pulse(); await flush(); await flush();
    assert.equal(t.peer.verified(), false); assert.equal(t.revocations(), 1);
  } finally { await t.peer.close(); }
});
test('Profile2 packets require an explicit trusted native profile selection', async () => {
  const text = JSON.stringify({ p: 'palisade.assist.control', v: 2, session: 'session_fixture_0001', kind: 'input-text', text: '日本語' });
  for (const inputProfile of ['legacy', 'input2']) {
    let received = 0;
    const t = await setup({ inputProfile, onPacket() { received++; } });
    try {
      t.message({ type: 'packet', text }); await flush();
      assert.equal(received, inputProfile === 'input2' ? 1 : 0);
      assert.equal(t.revocations(), inputProfile === 'input2' ? 0 : 1);
    } finally { await t.peer.close(); }
  }
});
test('a short queued packet cannot cross the peer-proof deadline before dispatch', async () => {
  let packets = 0;
  const t = await setup({ onPacket() { packets++; } });
  try {
    t.setTime(1490); t.message({ type: 'packet', text: packet });
    t.setTime(1510); // Only 20ms old, below the independent queue-age limit.
    await flush();
    assert.equal(packets, 0); assert.equal(t.revocations(), 1);
  } finally { await t.peer.close(); }
});
test('late native peer acceptance cannot admit an expired transport', async () => {
  const accepted = deferred();
  const t = await setup({ onObserved: () => accepted.promise });
  try {
    assert.equal(t.peer.verified(), false);
    t.setTime(2000); accepted.resolve(true); await flush();
    assert.equal(t.peer.verified(), false); assert.equal(t.revocations(), 1);
    assert.equal(t.sent().some(value => value.type === 'admit'), false);
  } finally { accepted.resolve(false); await t.peer.close(); }
});
