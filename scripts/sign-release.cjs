'use strict';
const { readFileSync, writeFileSync, statSync } = require('node:fs');
const { createHash, sign, createPublicKey } = require('node:crypto');
const path = require('node:path');
const { signedBytes, normalisePayload, verifyEnvelope } = require('../src/updates/release.cjs');
const { resolveTrust } = require('../src/updates/key-trust.cjs');
const { loadPrivateKey } = require('./signing-key.cjs');

const keyFile = process.env.PALISADE_UPDATE_PRIVATE_KEY_FILE;
const installer = process.argv[2];
const outputDir = process.argv[3] || 'out';
if (!keyFile || !installer) {
  console.error('Usage: set PALISADE_UPDATE_PRIVATE_KEY_FILE and run node scripts/sign-release.cjs <installer-path> [output-directory]');
  process.exit(1);
}
const pkg = require('../package.json');
const data = readFileSync(installer);
const size = statSync(installer).size;
if (data.length !== size) throw new Error('Installer changed while reading.');
const payload = normalisePayload({ schema: 1, channel: 'stable', version: pkg.version,
  filename: path.basename(installer), size,
  sha512: createHash('sha512').update(data).digest('base64'),
  publishedAt: new Date().toISOString(),
});
const privateKey = loadPrivateKey(keyFile);
const trusted = createPublicKey(readFileSync(path.resolve('assets/update-public-key.pem')));
const transitions = JSON.parse(readFileSync(path.resolve('assets/update-key-transitions.json'), 'utf8'));
const active = resolveTrust(trusted, transitions, pkg.version);
if (!active.equals(createPublicKey(privateKey))) {
  throw new Error('Signing key is not authorized by the pinned key and packaged transitions.');
}
const legacyKey = process.env.PALISADE_UPDATE_LEGACY_PRIVATE_KEY_FILE
  ? loadPrivateKey(process.env.PALISADE_UPDATE_LEGACY_PRIVATE_KEY_FILE) : privateKey;
if (legacyKey.asymmetricKeyType !== 'ed25519' || !trusted.equals(createPublicKey(legacyKey))) {
  throw new Error('The original private key is required for clients that still trust the original key.');
}
const signature = sign(null, signedBytes(payload), legacyKey).toString('base64');
const dest = path.join(outputDir, 'release.json');
const modern = normalisePayload({ ...payload, schema: 2, sha256: createHash('sha256').update(data).digest('hex') });
const envelope = { payload: modern, signature: sign(null, signedBytes(modern), legacyKey).toString('base64') };
const current = normalisePayload({ ...modern, schema: 3,
  downloadUrl: require('../src/updates/release.cjs').releaseAssetUrl(pkg.version, payload.filename) });
const currentEnvelope = { payload: current, signature: sign(null, signedBytes(current), privateKey).toString('base64'), transitions };
verifyEnvelope(envelope, trusted);
verifyEnvelope({ payload, signature }, trusted);
verifyEnvelope(currentEnvelope, trusted, transitions);
writeFileSync(dest, JSON.stringify({ payload, signature }, null, 2) + '\n', { flag: 'w' });
writeFileSync(path.join(outputDir, 'release-v2.json'), JSON.stringify(envelope, null, 2) + '\n');
writeFileSync(path.join(outputDir, 'release-v3.json'), JSON.stringify(currentEnvelope, null, 2) + '\n');
writeFileSync(path.join(outputDir, 'SHA256SUMS'), `${modern.sha256}  ${modern.filename}\n`);
console.log(`Signed manifest: ${dest}`);
