'use strict';
// Closed trusted-main/native vocabulary. This is not a hosted-page API.
const { NATIVE_ERRORS } = require('./native-protocol.cjs');
const { FP, TOKEN } = require('./remote-input-policy.cjs');
const exact = (value, fields) => value && typeof value === 'object' && !Array.isArray(value) &&
  Object.keys(value).sort().join(',') === [...fields].sort().join(',');
const ATTEMPT = /^[A-Za-z0-9_-]{43}$/;
const matches = (value, pattern) => typeof value === 'string' && pattern.test(value);
function operation(value) {
  const fields = { inspect: ['grant'], 'grant-state': ['grantId'], prepare: ['grantId'], seal: ['attempt', 'ownFingerprint'],
    observed: ['attempt', 'fingerprint'], check: ['attempt'], close: ['attempt'], packet: ['attempt', 'text', 'ingressNs'] }[value?.action];
  if (!fields || !exact(value, ['action', 'session', ...fields]) || !matches(value.session, TOKEN) ||
      (['prepare', 'grant-state'].includes(value.action) ? !matches(value.grantId, TOKEN) :
        value.action === 'inspect' ? typeof value.grant !== 'string' || Buffer.byteLength(value.grant) > 4096 : !matches(value.attempt, ATTEMPT))) return false;
  if (value.action === 'seal' && !matches(value.ownFingerprint, FP)) return false;
  if (value.action === 'observed' && !matches(value.fingerprint, FP)) return false;
  if (value.action === 'packet' && (typeof value.text !== 'string' || !value.text || Buffer.byteLength(value.text) > 16384 ||
      typeof value.ingressNs !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value.ingressNs) ||
      BigInt(value.ingressNs) > 0xffffffffffffffffn)) return false;
  return true;
}
function answer(value, action) {
  if (value?.outcome === 'error') return exact(value, ['outcome', 'code']) && NATIVE_ERRORS.includes(value.code);
  if (!exact(value, ['outcome', 'result']) || value.outcome !== 'ok') return false;
  const result = value.result;
  if (action === 'inspect') return exact(result, ['disposition', 'generation']) &&
    ['current', 'additional', 'replacement'].includes(result.disposition) && matches(result.generation, ATTEMPT);
  if (action === 'grant-state') return exact(result, ['generation']) && matches(result.generation, ATTEMPT);
  if (action === 'prepare') return exact(result, ['attempt', 'helperTransport', 'profile', 'sourceOperation', 'generation', 'resources', 'resourceChannels']) &&
    Array.isArray(result.resourceChannels) && result.resourceChannels.length <= 2 &&
    new Set(result.resourceChannels).size === result.resourceChannels.length && result.resourceChannels.every(value => ['clipboard', 'file'].includes(value)) &&
    result.resources === (result.resourceChannels.length > 0) &&
    typeof result.resources === 'boolean' && (result.profile !== 'resources' || result.resources) &&
    matches(result.generation, ATTEMPT) && matches(result.attempt, ATTEMPT) && matches(result.helperTransport, FP) && matches(result.sourceOperation, TOKEN) && ['legacy', 'input2', 'resources'].includes(result.profile);
  if (action === 'seal') return exact(result, ['transportSignature']) && matches(result.transportSignature, /^[A-Za-z0-9_-]{86}$/);
  if (['observed', 'check'].includes(action)) return exact(result, ['ready', 'inputReady', 'inputAllowed']) && typeof result.ready === 'boolean' &&
    typeof result.inputReady === 'boolean' && typeof result.inputAllowed === 'boolean' && (!result.inputReady || (result.ready && result.inputAllowed));
  return ['packet', 'close'].includes(action) && exact(result, []);
}
module.exports = { operation, answer };
