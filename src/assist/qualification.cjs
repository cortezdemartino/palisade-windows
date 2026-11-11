'use strict';
// Public build configuration only. Never read credentials, trust pins or origins
// from a page request, launch argument or runtime environment variable.
function validateQualification(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      Object.keys(value).sort().join(',') !== 'issuerFingerprint,namespace,origin') throw Error('Invalid Assist qualification profile');
  let url;
  try { url = new URL(value.origin); } catch { throw Error('Invalid Assist qualification origin'); }
  if (url.protocol !== 'https:' || url.origin !== value.origin || url.username || url.password ||
      !/^[a-z0-9.-]+$/.test(url.hostname) || url.hostname.replace(/\.+$/, '') === 'palisade.onyxops.org' || url.port === '0' ||
      !/^[a-f0-9]{64}$/.test(value.issuerFingerprint) || !/^[a-z][a-z0-9-]{2,31}$/.test(value.namespace)) {
    throw Error('Invalid Assist qualification profile');
  }
  return Object.freeze({ origin: value.origin, issuerFingerprint: value.issuerFingerprint, namespace: value.namespace });
}
let qualification = null;
try { qualification = validateQualification(require('./qualification.json')); }
catch (error) { if (error.code !== 'MODULE_NOT_FOUND' || !error.message.includes("'./qualification.json'")) throw error; }
module.exports = { qualification, validateQualification };
