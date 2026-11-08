'use strict';
// Non-authorizing policy mirror of docs/51 §4.4. The actual native executor
// must apply the same gate before permitted OS input; this module cannot inject.
function error(code) { return Object.assign(Error(code), { code }); }
function createInputAgeGuard(maxAgeMs = 300) {
  if (!Number.isSafeInteger(maxAgeMs) || maxAgeMs < 0) throw error('invalid-message');
  let baseline = Infinity, lastT = -1;
  return (t, arrival, release) => {
    if (!Number.isSafeInteger(t) || t < 0 || !Number.isFinite(arrival) || typeof release !== 'boolean')
      throw error('invalid-message');
    if (t < lastT) throw error('replay');
    lastT = t;
    const delay = arrival - t;
    baseline = Math.min(baseline,delay);
    if (!release && delay-baseline > maxAgeMs) throw error('stale-input');
  };
}
module.exports={createInputAgeGuard};
