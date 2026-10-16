'use strict';
const { readFileSync, writeFileSync, statSync } = require('node:fs');
const { createHash, sign, createPrivateKey, createPublicKey } = require('node:crypto');
const path = require('node:path');
const { signedBytes, normalisePayload, verifyEnvelope } = require('../src/updates/release.cjs');

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
const privateKey = createPrivateKey(readFileSync(keyFile));
if (privateKey.asymmetricKeyType !== 'ed25519') throw new Error('Expected an Ed25519 private key.');
const trusted = createPublicKey(readFileSync(path.resolve('assets/update-public-key.pem')));
if (!privateKey.asymmetricKeyType || !trusted.equals(createPublicKey(privateKey))) {
  throw new Error('Release private key does not match the client-pinned public key.');
}
const signature = sign(null, signedBytes(payload), privateKey).toString('base64');
const dest = path.join(outputDir, 'release.json');
writeFileSync(dest, JSON.stringify({ payload, signature }, null, 2) + '\n', { flag: 'w' });
const modern = normalisePayload({ ...payload, schema: 2, sha256: createHash('sha256').update(data).digest('hex') });
const envelope = { payload: modern, signature: sign(null, signedBytes(modern), privateKey).toString('base64') };
verifyEnvelope(envelope, trusted);
verifyEnvelope({ payload, signature }, trusted);
writeFileSync(path.join(outputDir, 'release-v2.json'), JSON.stringify(envelope, null, 2) + '\n');
writeFileSync(path.join(outputDir, 'SHA256SUMS'), `${modern.sha256}  ${modern.filename}\n`);
console.log(`Signed manifest: ${dest}`);
