'use strict';
const { generateKeyPairSync } = require('node:crypto');
const { writeFileSync, existsSync, mkdirSync } = require('node:fs');
const path = require('node:path');
const dest = path.resolve(process.argv[2] || 'secrets/update-private-key.pem');
const pub = path.resolve('assets/update-public-key.pem');
if (existsSync(dest) || existsSync(pub)) {
  console.error('Refusing to overwrite an existing release signing key.'); process.exit(1);
}
const { privateKey, publicKey } = generateKeyPairSync('ed25519', {
  privateKeyEncoding: { type: 'pkcs8', format: 'pem' },
  publicKeyEncoding: { type: 'spki', format: 'pem' },
});
mkdirSync(path.dirname(dest), { recursive: true });
writeFileSync(dest, privateKey, { mode: 0o600, flag: 'wx' });
writeFileSync(pub, publicKey, { mode: 0o644, flag: 'wx' });
console.log(`Private update key created: ${dest} (keep offline; never commit or distribute)`);
console.log(`Pinned public verification key created: ${pub} (commit before release)`);
