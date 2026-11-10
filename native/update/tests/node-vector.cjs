'use strict';
// Cross-language fixture: fresh keys stay in memory; output contains public data only.
const { generateKeyPairSync, createHash, sign } = require('node:crypto');
const { normalisePayload, signedBytes, releaseAssetUrl } = require('../../../src/updates/release.cjs');
const { fingerprint, transitionBytes } = require('../../../src/updates/key-trust.cjs');
const { UUID } = require('builder-util-runtime');
const root = generateKeyPairSync('ed25519'), successor = generateKeyPairSync('ed25519');
const payload = normalisePayload({ schema: 3, channel: 'stable', version: '0.3.4',
  filename: 'Palisade-Setup-0.3.4-x64.exe', size: 1, publishedAt: '2026-11-10T00:00:00.000Z',
  sha256: createHash('sha256').update('x').digest('hex'), sha512: createHash('sha512').update('x').digest('base64'),
  downloadUrl: releaseAssetUrl('0.3.4', 'Palisade-Setup-0.3.4-x64.exe') });
const transition = { schema: 1, from: fingerprint(root.publicKey),
  toPublicKey: successor.publicKey.export({ type: 'spki', format: 'pem' }), minVersion: '0.3.4' };
const history = [{ payload: transition, signature: sign(null, transitionBytes(transition), root.privateKey).toString('base64') }];
process.stdout.write(JSON.stringify({ root: root.publicKey.export({ type: 'spki', format: 'pem' }), history,
  uninstallGuid: UUID.v5(require('../../../package.json').build.appId, UUID.parse('50e065bc-3134-11e6-9bab-38c9862bdaf3')),
  release: { payload, transitions: history, signature: sign(null, signedBytes(payload), successor.privateKey).toString('base64') } }));
