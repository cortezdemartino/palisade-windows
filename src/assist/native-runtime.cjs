'use strict';
// One retained native hierarchy per desktop launch. A lost document ends it;
// no renderer action can replace its signer or restore its authority.
const { openLiveChannel } = require('./live-channel.cjs');
const { NativeBridgeError } = require('./native-protocol.cjs');
const launches = new WeakSet();
function createNativeRuntime({ lifecycle, binary, manifestFile, probe = false, onStatus = () => {} }) {
  if (!lifecycle || launches.has(lifecycle)) throw Error('Assist runtime already reserved');
  launches.add(lifecycle);
  let ended = false, failed = false, closing = null, document = null;
  const report = text => { try { onStatus(text); } catch {} };
  report('Starting native endpoint');
  const opening = openLiveChannel({ lifecycle, binary, manifestFile, probe, ownedTree: true, endpoint: true });
  void opening.then(channel => {
    if (!ended) report('Enrollment required');
    void channel.wait().then(() => { report('Session ended'); }, () => { failed = true; report('Cleanup failed'); });
  }, () => { failed = true; report('Native endpoint unavailable'); });
  function stopAll() {
    ended = true;
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
    const cancel = () => { void stopAll().catch(() => {}); };
    scope.signal.addEventListener('abort', cancel, { once: true });
    try {
      const channel = await opening;
      if (ended || failed || scope.signal.aborted) throw new NativeBridgeError('stopped');
      if (request.op === 'enroll.key') report('Awaiting local trust confirmation');
      if (request.op === 'enroll.prove') report('Awaiting native consent');
      const result = await channel.request(request, scope.generation);
      if (ended || scope.signal.aborted) throw new NativeBridgeError('stopped');
      if (request.op === 'enroll.prove') report('Local enrollment confirmed; awaiting service verification');
      return result;
    } catch (error) {
      if (error.code === 'consent-denied') report('Consent denied');
      throw error;
    } finally { scope.signal.removeEventListener('abort', cancel); }
  }
  return Object.freeze({ dispatch, stopAll, ready: opening.then(() => undefined) });
}
module.exports = { createNativeRuntime };
