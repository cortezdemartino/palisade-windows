'use strict';
const path = require('node:path');
const { readState, writeState } = require('./state-file.cjs');
const { UUID } = require('./native-helper.cjs');
const DIGEST = /^[a-f0-9]{64}$/;
function version(value) {
  return typeof value === 'string' && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value) &&
    value.split('.').every(part => Number.isSafeInteger(Number(part)));
}
function timestamp(value) {
  return Number.isSafeInteger(value) && value > 0 && value <= 8640000000000000;
}
function clean(value) {
  const stage = value?.stage;
  const checkedStage = stage && typeof stage.id === 'string' && UUID.test(stage.id) &&
    version(stage.from) && version(stage.version) && typeof stage.releaseSha256 === 'string' &&
    DIGEST.test(stage.releaseSha256) && ['download', 'staged', 'handoff'].includes(stage.phase) &&
    (stage.phase !== 'handoff' || (typeof stage.nonce === 'string' && DIGEST.test(stage.nonce)));
  return {
    schema: 1, lastCheck: timestamp(value?.lastCheck) ? value.lastCheck : 0,
    stage: checkedStage ? { id: stage.id, from: stage.from, version: stage.version, releaseSha256: stage.releaseSha256,
      phase: stage.phase, ...(stage.phase === 'handoff' ? { nonce: stage.nonce } : {}) } : null,
    history: Array.isArray(value?.history) ? value.history.slice(0, 12).filter(item =>
      version(item?.version) && timestamp(item.at) && ['installed', 'interrupted'].includes(item.result))
      .map(item => ({ version: item.version, at: item.at, result: item.result })) : [],
  };
}
function createJournal(directory) {
  const filename = path.join(directory, 'update-journal-v1.json');
  let state = clean(null);
  try {
    const value = readState(filename, 8192);
    if (value?.schema === 1) state = clean(value);
  } catch { /* Missing or malformed display state carries no update authority. */ }
  return {
    get: () => structuredClone(state),
    set: (patch) => {
      const next = clean({ ...state, ...patch });
      writeState(filename, next);
      state = next;
    },
  };
}
module.exports = { createJournal };
