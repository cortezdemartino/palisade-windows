'use strict';

const { createHash, createPublicKey, verify } = require('node:crypto');

function publicKey(value) {
  const key = value?.type === 'public' ? value : createPublicKey(value);
  if (key.asymmetricKeyType !== 'ed25519') throw new Error('Expected an Ed25519 public key.');
  return key;
}

function fingerprint(value) {
  return createHash('sha256').update(publicKey(value).export({ type: 'spki', format: 'der' })).digest('hex');
}

function normaliseTransition(value) {
  if (!value || Object.getPrototypeOf(value) !== Object.prototype ||
      Object.keys(value).sort().join(',') !== 'from,minVersion,schema,toPublicKey' ||
      value.schema !== 1 || !/^[a-f0-9]{64}$/.test(value.from) ||
      typeof value.toPublicKey !== 'string' || value.toPublicKey.length > 512 ||
      typeof value.minVersion !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value.minVersion)) {
    throw new Error('Invalid signing-key transition.');
  }
  const parts = value.minVersion.split('.').map(Number);
  if (parts.some((part) => !Number.isSafeInteger(part))) throw new Error('Invalid transition version.');
  return { schema: 1, from: value.from,
    toPublicKey: publicKey(value.toPublicKey).export({ type: 'spki', format: 'pem' }), minVersion: value.minVersion };
}

function transitionBytes(payload) {
  return Buffer.from('PALISADE-DESKTOP-KEY-TRANSITION-V1\n' + JSON.stringify(normaliseTransition(payload)));
}

function compareStable(a, b) {
  const left = a.split('.').map(Number), right = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return left[i] > right[i] ? 1 : -1;
  return 0;
}

/** Every delegation is signed by its predecessor, and cannot authorize earlier releases. */
function resolveTrust(root, transitions, releaseVersion) {
  if (!Array.isArray(transitions) || transitions.length > 8) throw new Error('Invalid transition chain.');
  if (typeof releaseVersion !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(releaseVersion) ||
      releaseVersion.split('.').some((part) => !Number.isSafeInteger(Number(part)))) {
    throw new Error('Invalid trust activation version.');
  }
  let key = publicKey(root);
  let floor = '0.0.0';
  const seen = new Set([fingerprint(key)]);
  for (const entry of transitions) {
    if (!entry || Object.getPrototypeOf(entry) !== Object.prototype ||
        Object.keys(entry).sort().join(',') !== 'payload,signature' ||
        typeof entry.signature !== 'string' || !/^[A-Za-z0-9+/]{86}==$/.test(entry.signature)) {
      throw new Error('Invalid signed key transition.');
    }
    const payload = normaliseTransition(entry.payload);
    const next = fingerprint(payload.toPublicKey);
    if (payload.from !== fingerprint(key) || seen.has(next) ||
        compareStable(payload.minVersion, floor) <= 0 ||
        compareStable(releaseVersion, payload.minVersion) < 0 ||
        !verify(null, transitionBytes(payload), key, Buffer.from(entry.signature, 'base64'))) {
      throw new Error('Signing-key transition authentication failed.');
    }
    floor = payload.minVersion;
    seen.add(next);
    key = publicKey(payload.toPublicKey);
  }
  return key;
}

function requireTrustedPrefix(transitions, installedTransitions) {
  if (!Array.isArray(transitions) || !Array.isArray(installedTransitions) ||
      transitions.length < installedTransitions.length) throw new Error('Signing-key rollback refused.');
  for (let i = 0; i < installedTransitions.length; i++) {
    if (JSON.stringify(normaliseTransition(transitions[i]?.payload)) !==
          JSON.stringify(normaliseTransition(installedTransitions[i].payload))) {
      throw new Error('Signing-key history cannot be replaced.');
    }
  }
}

module.exports = { publicKey, fingerprint, normaliseTransition, transitionBytes, resolveTrust, requireTrustedPrefix };
