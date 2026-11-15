'use strict';
// Orchestration only. No native endpoint, consent, input or media acceptance.
const test = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), vm = require('node:vm');
const { NativeBridgeError } = require('../src/assist/native-protocol.cjs');
const turn = () => new Promise(resolve => setImmediate(resolve));
function deferred() { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; }
function setup({ enabled = true } = {}) {
  const requests = [], selections = [], order = [], peerRequests = [];
  let peerOptions, peerClosed = 0, nativePeer = async operation => operation.action === 'prepare'
    ? { attempt: 'A'.repeat(43), helperTransport: 'sha-256 ' + Array(32).fill('AB').join(':'), profile: 'legacy', generation: 'G'.repeat(43), sourceOperation: 'capture_fixture_01' }
    : operation.action === 'seal' ? { transportSignature: 'B'.repeat(86) } : ['observed', 'check'].includes(operation.action) ? { ready: true } : {};
  let options, closed = 0, disposed = 0, fence = null, onStart = async () => ({ operation: 'capture_fixture_01' });
  const lost = new AbortController();
  const channel = {
    peer: async (operation, document) => { peerRequests.push({ operation, document }); return nativePeer(operation); },
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
      if (name === './remote-input-peer.cjs') return { createOwnedInputPeer: async value => {
        peerOptions = value;
        let closed = false;
        return { offer: async () => ({ sdp: 'owned-answer', fingerprint: 'sha-256 ' + Array(32).fill('CD').join(':') }),
          candidate: async () => { order.push('owned-candidate'); },
          close: async () => { if (!closed) { closed = true; peerClosed++; await value.onClosed('stopped'); } } };
      } };
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
    peerOptions: () => peerOptions, peerClosed: () => peerClosed, peerRequests, nativePeer: value => { nativePeer = value; },
    lose: () => lost.abort(), disposed: () => disposed,
    fence: value => { fence = value; }, onStart: value => { onStart = value; } };
}
const peerOffer = { op: 'peer.offer', session: 'session_fixture_0001', grantId: 'grant_fixture_0001', sdp: 'helper-offer' };
test('fresh input offers can connect only after a new native grant exchange and old callbacks cannot stop a successor', async () => {
  const h = setup(); await h.runtime.ready;
  let prepared = 0;
  h.nativePeer(async operation => {
    if (operation.action === 'inspect') return { disposition: 'replacement', generation: 'G'.repeat(43) };
    if (operation.action === 'grant-state') return { generation: 'H'.repeat(43) };
    if (operation.action === 'prepare') return { attempt: (++prepared === 1 ? 'A' : 'B').repeat(43), helperTransport: 'sha-256 ' + Array(32).fill('AB').join(':'), profile: 'legacy', generation: (prepared === 1 ? 'G' : 'H').repeat(43), sourceOperation: 'capture_fixture_01' };
    if (operation.action === 'seal') return { transportSignature: 'C'.repeat(86) };
    return ['check', 'observed'].includes(operation.action) ? { ready: true } : {};
  });
  await h.runtime.dispatch(peerOffer, h.scope());
  const old = h.peerOptions();
  await old.onObserved(old.helperTransport);
  await h.runtime.dispatch({ op: 'peer.close', session: peerOffer.session }, h.scope());
  await assert.rejects(h.runtime.dispatch(peerOffer, h.scope()), { code: 'stale-operation' });
  await h.runtime.dispatch({ op: 'grant.offer', session: peerOffer.session,
    grant: JSON.stringify({ grant: { grantId: 'fresh_grant_0001', capability: 'control_mouse' } }) }, h.scope());
  await h.runtime.dispatch({ ...peerOffer, grantId: 'fresh_grant_0001' }, h.scope());
  const fresh = h.peerOptions(); await fresh.onObserved(fresh.helperTransport);
  const genericStops = h.requests.filter(value => value.request.op === 'peer.close').length;
  await old.onClosed();
  await assert.rejects(old.onPacket('old gesture', null, '123'), { code: 'stopped' });
  await fresh.onPacket('fresh gesture', null, '456');
  assert.equal(h.peerRequests.at(-1).operation.attempt, 'B'.repeat(43));
  assert.equal(h.peerRequests.at(-1).operation.text, 'fresh gesture');
  assert.equal(h.requests.filter(value => value.request.op === 'peer.close').length, genericStops);
  assert(h.peerRequests.filter(value => value.operation.action === 'close').every(value => value.operation.attempt === 'A'.repeat(43)));
  await h.runtime.stopAll();
});
test('a same-generation permission ACK and a refused replacement leave the working peer intact', async () => {
  const h = setup(); await h.runtime.ready;
  await h.runtime.dispatch(peerOffer, h.scope());
  const active = h.peerOptions();
  h.nativePeer(async operation => {
    if (operation.action === 'inspect') return { disposition: 'additional', generation: 'G'.repeat(43) };
    if (operation.action === 'grant-state') return { generation: 'G'.repeat(43) };
    return ['check', 'observed'].includes(operation.action) ? { ready: true } : {};
  });
  await h.runtime.dispatch({ op: 'grant.offer', session: peerOffer.session,
    grant: JSON.stringify({ grant: { grantId: 'keyboard_grant_01', capability: 'control_keyboard' } }) }, h.scope());
  assert.equal(h.peerClosed(), 0);
  await active.onPacket('current', null, '123');
  h.nativePeer(async operation => {
    if (operation.action === 'inspect') throw new NativeBridgeError('transport-mismatch');
    return {};
  });
  await assert.rejects(h.runtime.dispatch({ op: 'grant.offer', session: peerOffer.session,
    grant: JSON.stringify({ grant: { grantId: 'untrusted_grant_01', capability: 'control_mouse' } }) }, h.scope()), { code: 'transport-mismatch' });
  assert.equal(h.peerClosed(), 0);
  await active.onPacket('still current', null, '456');
  await h.runtime.stopAll();
});
test('a new Host Stop during fresh ACK verification cannot be cleared by the older request', async () => {
  const h = setup(); await h.runtime.ready;
  await h.runtime.dispatch(peerOffer, h.scope());
  await h.runtime.dispatch({ op: 'peer.close', session: peerOffer.session }, h.scope());
  const entered = deferred(), state = deferred();
  h.nativePeer(async operation => {
    if (operation.action === 'inspect') return { disposition: 'replacement', generation: 'G'.repeat(43) };
    if (operation.action === 'grant-state') { entered.resolve(); await state.promise; return { generation: 'H'.repeat(43) }; }
    return {};
  });
  const fresh = assert.rejects(h.runtime.dispatch({ op: 'grant.offer', session: peerOffer.session,
    grant: JSON.stringify({ grant: { grantId: 'fresh_grant_0001', capability: 'control_mouse' } }) }, h.scope()), { code: 'stopped' });
  await entered.promise;
  await h.runtime.dispatch({ op: 'peer.close', session: peerOffer.session }, h.scope());
  state.resolve(); await fresh;
  await assert.rejects(h.runtime.dispatch(peerOffer, h.scope()), { code: 'stale-operation' });
  assert.equal(h.requests.filter(v => v.request.op === 'peer.close').length, 2);
  await h.runtime.stopAll();
});
test('independent resource grant exchange cannot reopen a stopped input association', async () => {
  const h = setup(); await h.runtime.ready;
  await h.runtime.dispatch(peerOffer, h.scope());
  await h.runtime.dispatch({ op: 'peer.close', session: peerOffer.session }, h.scope());
  await h.runtime.dispatch({ op: 'grant.offer', session: peerOffer.session,
    grant: JSON.stringify({ grant: { grantId: 'resource_grant_01', capability: 'clipboard_to_host' } }) }, h.scope());
  await assert.rejects(h.runtime.dispatch(peerOffer, h.scope()), { code: 'stale-operation' });
  assert.equal(h.peerRequests.filter(value => value.operation.action === 'prepare').length, 1);
  await h.runtime.stopAll();
});
test('source stopped during native preparation cannot construct a late peer', async () => {
  const h = setup(); await h.runtime.ready;
  const prepared = deferred(), entered = deferred();
  h.nativePeer(async () => { entered.resolve(); await prepared.promise;
    return { attempt: 'A'.repeat(43), helperTransport: 'sha-256 ' + Array(32).fill('AB').join(':'), profile: 'legacy', generation: 'G'.repeat(43), sourceOperation: 'capture_fixture_01' };
  });
  const offering = assert.rejects(h.runtime.dispatch(peerOffer, h.scope()), { code: 'stopped' });
  await entered.promise;
  await h.runtime.dispatch({ op: 'capture.stop', operation: 'capture_fixture_01' }, h.scope());
  prepared.resolve(); await offering;
  assert.equal(h.peerOptions(), undefined);
  await h.runtime.stopAll();
});
test('unknown or retired source cancellation cannot close a peer on another source', async () => {
  const h = setup(); await h.runtime.ready;
  await h.runtime.dispatch(peerOffer, h.scope());
  await h.runtime.dispatch({ op: 'capture.stop', operation: 'capture_retired_01' }, h.scope());
  assert.equal(h.peerClosed(), 0);
  await h.peerOptions().onObserved(h.peerOptions().helperTransport);
  await h.peerOptions().onPacket('fresh-packet', null, '123');
  assert.equal(h.peerRequests.at(-1).operation.action, 'packet');
  await h.runtime.stopAll();
});
test('native executor readiness is polled without optimistic packet admission', async () => {
  const h = setup(); await h.runtime.ready;
  let checks = 0;
  h.nativePeer(async operation => {
    if (operation.action === 'prepare') return { attempt: 'A'.repeat(43), helperTransport: 'sha-256 ' + Array(32).fill('AB').join(':'), profile: 'legacy', generation: 'G'.repeat(43), sourceOperation: 'capture_fixture_01' };
    if (operation.action === 'seal') return { transportSignature: 'B'.repeat(86) };
    if (operation.action === 'check') return { ready: ++checks >= 2 };
    return { ready: false };
  });
  await h.runtime.dispatch(peerOffer, h.scope());
  assert.equal(await h.peerOptions().onObserved(h.peerOptions().helperTransport), true);
  assert.equal(checks, 2);
  await h.runtime.stopAll();
});
test('owned peer uses native binding and one-shot transport seal before observed certificate admission', async () => {
  const h = setup(); await h.runtime.ready;
  const scope = h.scope();
  const result = await h.runtime.dispatch(peerOffer, scope);
  assert.equal(result.sdp, 'owned-answer'); assert.equal(result.transportSignature, 'B'.repeat(86));
  assert.equal(h.peerOptions().helperTransport, 'sha-256 ' + Array(32).fill('AB').join(':'));
  assert.equal(h.peerOptions().inputProfile, 'legacy');
  await h.peerOptions().onObserved(h.peerOptions().helperTransport);
  await h.peerOptions().onPacket('original-bytes', scope.signal, '123456789');
  assert.deepEqual(h.peerRequests.map(v => v.operation.action), ['prepare', 'seal', 'observed', 'packet']);
  assert.equal(h.peerRequests.at(-1).operation.ingressNs, '123456789');
  assert(h.peerRequests.every(v => v.document === 7 && v.operation.session === peerOffer.session));
  assert.equal(h.peerRequests[1].operation.ownFingerprint, 'sha-256 ' + Array(32).fill('CD').join(':'));
  await assert.rejects(h.runtime.dispatch(peerOffer, h.scope()), { code: 'stale-operation' });
  await h.runtime.stopAll(); assert.equal(h.peerClosed(), 1);
});
test('certificate observation racing answer signing waits for this exact seal', async () => {
  const h = setup(); await h.runtime.ready;
  const seal = deferred(), entered = deferred();
  h.nativePeer(async operation => {
    if (operation.action === 'prepare') return { attempt: 'A'.repeat(43), helperTransport: 'sha-256 ' + Array(32).fill('AB').join(':'), profile: 'legacy', generation: 'G'.repeat(43), sourceOperation: 'capture_fixture_01' };
    if (operation.action === 'seal') { entered.resolve(); await seal.promise; return { transportSignature: 'B'.repeat(86) }; }
    return { ready: true };
  });
  const offering = h.runtime.dispatch(peerOffer, h.scope());
  await entered.promise;
  let admitted = false;
  const observing = h.peerOptions().onObserved(h.peerOptions().helperTransport).then(() => { admitted = true; });
  await turn(); assert.equal(admitted, false);
  assert.equal(h.peerRequests.some(v => v.operation.action === 'observed'), false);
  seal.resolve(); await offering; await observing;
  assert.equal(admitted, true);
  await h.runtime.stopAll();
});
test('Stop fences the owned peer while a seal or native cleanup is still pending', async () => {
  const h = setup(); await h.runtime.ready;
  const seal = deferred(), entered = deferred();
  h.nativePeer(async operation => {
    if (operation.action === 'prepare') return { attempt: 'A'.repeat(43), helperTransport: 'sha-256 ' + Array(32).fill('AB').join(':'), profile: 'legacy', generation: 'G'.repeat(43), sourceOperation: 'capture_fixture_01' };
    if (operation.action === 'seal') { entered.resolve(); await seal.promise; return { transportSignature: 'B'.repeat(86) }; }
    return { ready: true };
  });
  const offering = assert.rejects(h.runtime.dispatch(peerOffer, h.scope()), { code: 'stopped' });
  await entered.promise;
  const observing = assert.rejects(h.peerOptions().onObserved(h.peerOptions().helperTransport), { code: 'stopped' });
  await h.runtime.dispatch({ op: 'peer.close', session: peerOffer.session }, h.scope());
  assert.equal(h.peerClosed(), 1);
  seal.resolve(); await offering; await observing;
  assert.equal(h.peerRequests.some(v => v.operation.action === 'observed'), false);
  await h.runtime.stopAll();
});
test('native refusal cannot create a peer, and document or source loss closes transport admission', async () => {
  const h = setup(); await h.runtime.ready;
  h.nativePeer(async () => { throw new NativeBridgeError('binding-mismatch'); });
  await assert.rejects(h.runtime.dispatch(peerOffer, h.scope()), { code: 'binding-mismatch' });
  assert.equal(h.peerOptions(), undefined);
  await h.runtime.stopAll();
  for (const reason of ['source', 'document']) {
    const h = setup(); await h.runtime.ready;
    await h.runtime.dispatch(peerOffer, h.scope());
    if (reason === 'source') await h.runtime.dispatch({ op: 'capture.stop', operation: 'capture_fixture_01' }, h.scope());
    else await assert.rejects(h.runtime.dispatch({ op: 'status' }, { ...h.scope(), generation: 8 }), { code: 'stale-operation' });
    assert.equal(h.peerClosed(), 1);
    await assert.rejects(h.peerOptions().onPacket('late-packet', null, '123'), { code: 'stopped' });
    await h.runtime.stopAll();
  }
});
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
