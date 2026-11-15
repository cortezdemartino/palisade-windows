'use strict';
// Closed trusted-main/native vocabulary. This is not a hosted-page API.
const { NATIVE_ERRORS } = require('./native-protocol.cjs');
const { FP, TOKEN } = require('./remote-input-policy.cjs');
const exact = (value, fields) => value && typeof value === 'object' && !Array.isArray(value) &&
  Object.keys(value).sort().join(',') === [...fields].sort().join(',');
const ATTEMPT = /^[A-Za-z0-9_-]{43}$/;
const matches = (value, pattern) => typeof value === 'string' && pattern.test(value);
function operation(value) {
  const fields = { prepare: ['grantId'], seal: ['attempt', 'ownFingerprint'],
    observed: ['attempt', 'fingerprint'], check: ['attempt'], packet: ['attempt', 'text', 'ingressNs'] }[value?.action];
  if (!fields || !exact(value, ['action', 'session', ...fields]) || !matches(value.session, TOKEN) ||
      (value.action === 'prepare' ? !matches(value.grantId, TOKEN) : !matches(value.attempt, ATTEMPT))) return false;
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
  if (action === 'prepare') return exact(result, ['attempt', 'helperTransport', 'profile', 'sourceOperation']) &&
    matches(result.attempt, ATTEMPT) && matches(result.helperTransport, FP) && matches(result.sourceOperation, TOKEN) && ['legacy', 'input2'].includes(result.profile);
  if (action === 'seal') return exact(result, ['transportSignature']) && matches(result.transportSignature, /^[A-Za-z0-9_-]{86}$/);
  if (['observed', 'check'].includes(action)) return exact(result, ['ready']) && typeof result.ready === 'boolean';
  return action === 'packet' && exact(result, []);
}
module.exports = { operation, answer };
