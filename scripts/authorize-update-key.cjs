'use strict';

const { readFileSync, writeFileSync } = require('node:fs');
const { createPublicKey, sign } = require('node:crypto');
const path = require('node:path');
const { fingerprint, normaliseTransition, transitionBytes, resolveTrust } = require('../src/updates/key-trust.cjs');
const { loadPrivateKey } = require('./signing-key.cjs');

// Input private keys stay external. Output contains only public keys and signatures.
const [nextPublicFile, minVersion, outputFile] = process.argv.slice(2);
if (!process.env.PALISADE_UPDATE_PRIVATE_KEY_FILE || !nextPublicFile || !minVersion || !outputFile) {
  throw new Error('Set PALISADE_UPDATE_PRIVATE_KEY_FILE to the current authorized private key; provide <next-public.pem> <minimum-version> <new-chain.json>.');
}
const root = readFileSync(path.resolve('assets/update-public-key.pem'));
const chain = JSON.parse(readFileSync(path.resolve('assets/update-key-transitions.json'), 'utf8'));
const current = resolveTrust(root, chain, minVersion);
const privateKey = loadPrivateKey(process.env.PALISADE_UPDATE_PRIVATE_KEY_FILE);
if (privateKey.asymmetricKeyType !== 'ed25519' || !current.equals(createPublicKey(privateKey))) {
  throw new Error('Transition must be signed by the currently authorized key.');
}
const payload = normaliseTransition({ schema: 1, from: fingerprint(current),
  toPublicKey: readFileSync(nextPublicFile, 'utf8'), minVersion });
const entry = { payload, signature: sign(null, transitionBytes(payload), privateKey).toString('base64') };
const next = [...chain, entry];
resolveTrust(root, next, minVersion);
writeFileSync(outputFile, JSON.stringify(next, null, 2) + '\n', { flag: 'wx' });
console.log(`Authorized public-key transition to ${fingerprint(payload.toPublicKey)} from ${minVersion}. Review and package this chain before signing releases with the successor.`);
