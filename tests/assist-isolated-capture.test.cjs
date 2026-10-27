'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { EventEmitter } = require('node:events');
const turn = () => new Promise((resolve) => setImmediate(resolve));
function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function fixture(enumerate, start) {
  const timers = [],
    contents = Object.assign(new EventEmitter(), {
      mainFrame: {},
      isDestroyed: () => false,
      getURL: () => 'https://palisade.onyxops.org/',
    }),
    parent = Object.assign(new EventEmitter(), {
      isDestroyed: () => false,
      isVisible: () => true,
      isMinimized: () => false,
    });
  let active = false,
    starts = 0,
    stops = 0;
  const hop = {
    start: async (source) => {
      starts++;
      active = true;
      return start ? start(source, starts) : { operation: String(starts), elementId: 'owned' };
    },
    stop: () => {
      stops++;
      active = false;
    },
    busy: () => active,
    dispose: () => {},
  };
  const context = {
    module: { exports: {} },
    AbortController,
    setTimeout: (callback) => {
      const timer = { callback, cleared: false, unref() {} };
      timers.push(timer);
      return timer;
    },
    clearTimeout: (timer) => {
      if (timer) timer.cleared = true;
    },
    require: (name) => {
      if (name === 'electron') return {};
      if (name === './source-picker.cjs') return {};
      if (name === './local-video-hop.cjs') return { createLocalVideoHop: () => hop };
      if (name === './capture.cjs') return require('../src/assist/capture.cjs');
      return require('../src/security-policy.cjs');
    },
  };
  vm.runInNewContext(
    fs.readFileSync(require.resolve('../src/assist/isolated-capture.cjs'), 'utf8'),
    context,
  );
  const capture = context.module.exports.createIsolatedCapture({
    getContents: () => contents,
    getWindow: () => parent,
    enabled: true,
    inspect: () => {},
    enumerate,
    choose: async () => 'window:11:0',
  });
  return { capture, timers, starts: () => starts, stops: () => stops };
}
const sources = [{ id: 'window:11:0', name: 'Owned fixture' }];
test('cancelled hung enumeration settles promptly and its timer cannot stop a successor', async () => {
  const old = deferred();
  let calls = 0;
  const h = fixture(() => (++calls === 1 ? old.promise : Promise.resolve(sources)));
  const first = assert.rejects(h.capture.request(), /cancelled/);
  await turn();
  h.capture.stop();
  await first;
  assert(h.timers[0].cleared);
  const next = await h.capture.request();
  assert.equal(next.operation, '1');
  h.timers[0].callback(); // Even a callback already queued before clearTimeout must be harmless.
  old.resolve(sources);
  await turn();
  assert.equal(h.capture.selected().operation, '1');
  assert(h.capture.busy());
  assert.equal(h.starts(), 1);
  assert.equal(h.stops(), 1);
  h.capture.dispose();
});
test('late completion of a cancelled start cannot tear down a newer capture', async () => {
  const old = deferred();
  const h = fixture(
    async () => sources,
    (_source, number) =>
      number === 1 ? old.promise : { operation: String(number), elementId: 'owned' },
  );
  const first = assert.rejects(h.capture.request(), /cancelled/);
  await turn();
  h.capture.stop();
  await first;
  await h.capture.request();
  old.resolve({ operation: '1', elementId: 'retired' });
  await turn();
  assert.equal(h.capture.selected().operation, '2');
  assert(h.capture.busy());
  assert.equal(h.stops(), 1);
  h.capture.dispose();
});
test('selection timeout cancels a hung request without a late source grant', async () => {
  const old = deferred();
  const h = fixture(() => old.promise);
  const result = assert.rejects(h.capture.request(), /cancelled/);
  h.timers[0].callback();
  await result;
  assert.equal(h.capture.busy(), false);
  assert.equal(h.capture.selected(), null);
  old.resolve(sources);
  await turn();
  assert.equal(h.starts(), 0);
  h.capture.dispose();
});
