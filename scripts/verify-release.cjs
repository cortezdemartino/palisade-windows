'use strict';
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { verifyEnvelope, verifyDownloadedFile } = require('../src/updates/release.cjs');
async function main() {
  const directory = process.argv[2] || 'out';
  const key = readFileSync(path.join(__dirname, '../assets/update-public-key.pem'));
  const legacy = verifyEnvelope(JSON.parse(readFileSync(path.join(directory, 'release.json'))), key);
  const modern = verifyEnvelope(JSON.parse(readFileSync(path.join(directory, 'release-v2.json'))), key);
  if (legacy.schema !== 1 || modern.schema !== 2 || modern.version !== require('../package.json').version ||
      legacy.version !== modern.version || legacy.sha512 !== modern.sha512 || legacy.size !== modern.size ||
      !await verifyDownloadedFile(path.join(directory, modern.filename), modern) ||
      readFileSync(path.join(directory, 'SHA256SUMS'), 'utf8') !== `${modern.sha256}  ${modern.filename}\n`) {
    throw new Error('Release artifacts failed verification.');
  }
  console.log(`Verified both signed manifests and installer for ${modern.version}`);
}
main().catch((error) => { console.error(error.message); process.exitCode = 1; });
