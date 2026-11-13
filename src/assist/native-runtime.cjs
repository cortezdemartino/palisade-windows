'use strict';
// One retained native hierarchy per desktop launch. A lost document ends it;
// no renderer action can replace its signer or restore its authority.
const { openLiveChannel } = require('./live-channel.cjs');
const { NativeBridgeError } = require('./native-protocol.cjs');
const { randomBytes } = require('node:crypto');
const launches = new WeakSet();
function createNativeRuntime({ lifecycle, binary, manifestFile, probe = false, captureOptions = null, onStatus = () => {} }) {
  if (!lifecycle || launches.has(lifecycle)) throw Error('Assist runtime already reserved');
  launches.add(lifecycle);
  let ended = false, failed = false, closing = null, document = null;
  const report = text => { try { onStatus(text); } catch {} };
  report('Starting native endpoint');
  const opening = openLiveChannel({ lifecycle, binary, manifestFile, probe, ownedTree: true, endpoint: true });
  let publication = null;
  const sourceFences = new Map();
  const inputFences = new Map();
  const fenceInput = session => {
    if (inputFences.has(session)) return inputFences.get(session);
    if (inputFences.size >= 128) {
      failed = true; void stopAll().catch(() => {});
      return Promise.reject(new NativeBridgeError('stopped'));
    }
    const work = opening.then(channel => channel.request({ p: 'palisade.assist.native', v: 1,
      id: randomBytes(16).toString('hex'), op: 'peer.close', session }, document));
    inputFences.set(session, work);
    void work.catch(() => { void stopAll().catch(() => {}); });
    return work;
  };
  const fenceSource = operation => {
    if (sourceFences.has(operation)) return sourceFences.get(operation);
    if (sourceFences.size >= 128) {
      failed = true; void stopAll().catch(() => {});
      return Promise.reject(new NativeBridgeError('stopped'));
    }
    // Send on the reserved safety budget; do not wait for the ordinary worker.
    const work = opening.then(channel => channel.request({ p: 'palisade.assist.native', v: 1,
      id: randomBytes(16).toString('hex'), op: 'capture.stop', operation }, document));
    sourceFences.set(operation, work);
    void work.catch(() => { void stopAll().catch(() => {}); });
    return work;
  };
  const configured = opening.then(async channel => {
    if (!captureOptions) return;
    if (!await channel.configureCapture(true)) throw new NativeBridgeError('capture-unavailable');
    if (ended) return;
    const { createCapturePublication } = require('./capture-publication.cjs');
    publication = createCapturePublication({ ...captureOptions, enabled: true,
      inspect: ports => channel.inspectLocalIce(ports),
      onSelect: async (selected, { publicOperation }) => {
        if (ended || failed) throw new NativeBridgeError('stopped');
        let bounds = null;
        if (selected.id.startsWith('screen:')) {
          const display = captureOptions.screen.getAllDisplays().find(value => String(value.id) === selected.display_id);
          if (!display) throw new NativeBridgeError('capture-unavailable');
          const physical = captureOptions.screen.dipToScreenRect(null, display.bounds);
          bounds = { ...physical, generation: 1 };
        }
        await channel.selectCapture({ operation: publicOperation, source: selected.id,
          display: selected.display_id || null, bounds }, document);
        if (ended || failed) throw new NativeBridgeError('stopped');
      },
      onInvalidate: (_snapshot, context) => fenceSource(context.operation),
      onFatal: () => { void stopAll().catch(() => {}); },
    });
  });
  void configured.catch(() => { failed = true; void stopAll().catch(() => {}); });
  void opening.then(channel => {
    if (!ended) report('Enrollment required');
    const lost = () => { void stopAll().catch(() => {}); };
    channel.signal?.addEventListener('abort', lost, { once: true });
    if (channel.signal?.aborted) lost();
    void channel.wait().then(() => {
      ended = true; stopMedia(); report('Session ended');
    }, () => {
      failed = true; ended = true; stopMedia(); report('Cleanup failed');
    }).finally(() => channel.signal?.removeEventListener('abort', lost)).catch(() => {});
  }, () => { failed = true; report('Native endpoint unavailable'); });
  function stopMedia() {
    try { publication?.dispose(); } catch { failed = true; }
  }
  function stopAll() {
    ended = true;
    // Destroy the app-owned media path before awaiting native retained cleanup.
    stopMedia();
    if (!closing) {
      report('Cleanup pending');
      closing = opening.then(channel => channel.close());
      void closing.catch(() => { failed = true; report('Cleanup failed'); });
    }
    return closing;
  }
  async function dispatch(request, scope) {
    if (ended || failed || scope.signal.aborted) throw new NativeBridgeError('stopped');
    if (document === null) document = scope.generation;
    if (document !== scope.generation) { await stopAll(); throw new NativeBridgeError('stale-operation'); }
    const captureRequest = request.op.startsWith('capture.');
    const cancel = () => {
      if (captureRequest) { void fenceSource(request.operation).catch(() => {}); }
      else if (['grant.offer', 'peer.offer', 'peer.candidate', 'peer.close'].includes(request.op)) {
        void fenceInput(request.session).catch(() => {});
      }
      else { void stopAll().catch(() => {}); }
    };
    scope.signal.addEventListener('abort', cancel, { once: true });
    try {
      const channel = await opening;
      await configured;
      if (ended || failed || scope.signal.aborted) throw new NativeBridgeError('stopped');
      if (request.op === 'enroll.key') report('Awaiting local trust confirmation');
      if (request.op === 'enroll.prove') report('Awaiting native consent');
      if (request.op === 'peer.close') { await fenceInput(request.session); return {}; }
      if (captureRequest && publication) {
        if (request.op === 'capture.start') {
          if (sourceFences.has(request.operation)) throw new NativeBridgeError('stale-operation');
          await channel.request(request, scope.generation);
          if (scope.signal.aborted) throw new NativeBridgeError('cancelled');
          try { return await publication.dispatch(request, scope); }
          catch (error) { await fenceSource(request.operation); throw error; }
        }
        if (request.op === 'capture.bind') {
          await channel.request(request, scope.generation);
          try { return await publication.dispatch(request, scope); }
          catch (error) { await fenceSource(request.operation); throw error; }
        }
        await fenceSource(request.operation);
        return publication.dispatch(request, scope);
      }
      const result = await channel.request(request, scope.generation);
      if (ended || scope.signal.aborted) throw new NativeBridgeError('stopped');
      if (request.op === 'enroll.prove') report('Local enrollment confirmed; awaiting service verification');
      return result;
    } catch (error) {
      if (error.code === 'consent-denied') report('Consent denied');
      throw error;
    } finally { scope.signal.removeEventListener('abort', cancel); }
  }
  return Object.freeze({ dispatch, stopAll, ready: configured.then(() => undefined) });
}
module.exports = { createNativeRuntime };
