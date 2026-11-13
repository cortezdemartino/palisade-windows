'use strict';
// Orchestration only. No native endpoint, consent, input or media acceptance.
const test = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), vm = require('node:vm');
const { NativeBridgeError } = require('../src/assist/native-protocol.cjs');
const turn = () => new Promise(resolve => setImmediate(resolve));
function deferred() { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; }
function setup({ enabled = true } = {}) {
  const requests = [], selections = [], order = [];
  let options, closed = 0, disposed = 0, fence = null, onStart = async () => ({ operation: 'capture_fixture_01' });
  const lost = new AbortController();
  const channel = {
    configureCapture: async value => { order.push(['configure', value]); return enabled; },
    request: async (request, document) => {
      requests.push({ request, document }); order.push(request.op);
      if (request.op === 'capture.stop' && fence) await fence.promise;
      return request.op === 'capture.start' ? { operation: request.operation } : {};
    },
    selectCapture: async (selection, document) => { selections.push({ selection, document }); },
    inspectLocalIce: async () => ({ candidates: [] }),
    wait: () => new Promise(() => {}), close: async () => { closed++; },
    signal: lost.signal,
  };
  const context = { module: { exports: {} }, AbortController,
    require(name) {
      if (name === './live-channel.cjs') return { openLiveChannel: async () => channel };
      if (name === './native-protocol.cjs') return { NativeBridgeError };
      if (name === './capture-publication.cjs') return { createCapturePublication: value => {
        options = value;
        return { dispose() { disposed++; }, dispatch: (...args) => onStart(...args) };
      } };
      return require(name);
    },
  };
  vm.runInNewContext(fs.readFileSync(require.resolve('../src/assist/native-runtime.cjs'), 'utf8'), context);
  const screen = { getAllDisplays: () => [{ id: 9, bounds: { x: -1000, y: 0, width: 1000, height: 800 } }],
    dipToScreenRect: (_window, bounds) => ({ x: bounds.x * 2, y: 0, width: 2000, height: 1600 }) };
  const runtime = context.module.exports.createNativeRuntime({ lifecycle: {}, captureOptions: { screen } });
  const scope = () => { const abort = new AbortController(); return { abort, signal: abort.signal, generation: 7, emit: () => true }; };
  return { runtime, requests, selections, order, scope, options: () => options, closed: () => closed,
    lose: () => lost.abort(), disposed: () => disposed,
    fence: value => { fence = value; }, onStart: value => { onStart = value; } };
}
test('qualified runtime verifies the selected physical display before publication, without first-monitor fallback', async () => {
  const h = setup(); await h.runtime.ready;
  const scope = h.scope();
  await h.runtime.dispatch({ op: 'hello' }, scope);
  await h.options().onSelect({ id: 'screen:2:0', display_id: '9' }, { publicOperation: 'capture_fixture_01' });
  assert.equal(h.selections[0].document, 7);
  assert.equal(JSON.stringify(h.selections[0].selection), JSON.stringify({ operation: 'capture_fixture_01', source: 'screen:2:0', display: '9',
    bounds: { x: -2000, y: 0, width: 2000, height: 1600, generation: 1 } }));
  await assert.rejects(h.options().onSelect({ id: 'screen:3:0', display_id: 'missing' }, { publicOperation: 'capture_fixture_02' }), { code: 'capture-unavailable' });
  assert.equal(h.selections.length, 1);
  await h.runtime.stopAll();
});
test('source stop waits for its native fence and does not end enrollment or the launch', async () => {
  const h = setup(); await h.runtime.ready;
  const scope = h.scope(); await h.runtime.dispatch({ op: 'hello' }, scope);
  const fence = deferred(); h.fence(fence);
  let completed = false;
  const work = h.runtime.dispatch({ op: 'capture.stop', operation: 'capture_fixture_01' }, h.scope()).then(() => { completed = true; });
  await turn(); assert.equal(completed, false); assert.equal(h.closed(), 0);
  fence.resolve(); await work;
  await h.runtime.dispatch({ op: 'status' }, scope);
  assert.equal(h.closed(), 0);
  await h.runtime.stopAll(); assert.equal(h.closed(), 1);
});
test('cancelled picker fences only its source and delayed selection cannot return success', async () => {
  const h = setup(); await h.runtime.ready;
  const selection = deferred(), scope = h.scope();
  h.onStart(async () => { await selection.promise; if (scope.signal.aborted) throw new NativeBridgeError('cancelled'); });
  const start = assert.rejects(h.runtime.dispatch({ op: 'capture.start', operation: 'capture_fixture_01' }, scope), { code: 'cancelled' });
  await turn(); scope.abort.abort(); selection.resolve(); await start;
  assert(h.requests.some(value => value.request.op === 'capture.stop'));
  assert.equal(h.closed(), 0);
  await h.runtime.dispatch({ op: 'status' }, h.scope());
  await h.runtime.stopAll();
});
test('native capture refusal closes the qualified runtime without opening a picker', async () => {
  const h = setup({ enabled: false });
  await assert.rejects(h.runtime.ready, { code: 'capture-unavailable' });
  await h.runtime.stopAll(); assert.equal(h.options(), undefined); assert.equal(h.closed(), 1);
});
test('native hierarchy loss immediately tears down owned media while cleanup is pending', async () => {
  const h = setup(); await h.runtime.ready;
  await h.runtime.dispatch({ op: 'capture.start', operation: 'capture_fixture_01' }, h.scope());
  assert.equal(h.disposed(), 0);
  h.lose();
  assert.equal(h.disposed(), 1);
  await assert.rejects(h.runtime.dispatch({ op: 'status' }, h.scope()), { code: 'stopped' });
  await h.runtime.stopAll();
});
test('an early source stop forbids a delayed start with the same operation ID', async () => {
  const h = setup(); await h.runtime.ready;
  await h.runtime.dispatch({ op: 'capture.stop', operation: 'capture_fixture_01' }, h.scope());
  await assert.rejects(h.runtime.dispatch({ op: 'capture.start', operation: 'capture_fixture_01' }, h.scope()), { code: 'stale-operation' });
  assert.equal(h.requests.filter(v => v.request.op === 'capture.start').length, 0);
  await h.runtime.stopAll();
});
test('source cancellation tombstones are bounded and exhaustion closes the runtime', async () => {
  const h = setup(); await h.runtime.ready;
  for (let n = 0; n < 128; n++) await h.runtime.dispatch({ op: 'capture.stop', operation: 'capture_fixture_' + n }, h.scope());
  await assert.rejects(h.runtime.dispatch({ op: 'capture.stop', operation: 'capture_overflow' }, h.scope()), { code: 'stopped' });
  await h.runtime.stopAll(); assert.equal(h.closed(), 1);
});
