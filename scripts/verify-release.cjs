'use strict';
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { verifyEnvelope, verifyDownloadedFile } = require('../src/updates/release.cjs');
async function main() {
  const directory = process.argv[2] || 'out';
  const key = readFileSync(path.join(__dirname, '../assets/update-public-key.pem'));
  const legacy = verifyEnvelope(JSON.parse(readFileSync(path.join(directory, 'release.json'))), key);
  const modern = verifyEnvelope(JSON.parse(readFileSync(path.join(directory, 'release-v2.json'))), key);
  const transitions = JSON.parse(readFileSync(path.join(__dirname, '../assets/update-key-transitions.json'), 'utf8'));
  const current = verifyEnvelope(JSON.parse(readFileSync(path.join(directory, 'release-v3.json'))), key, transitions);
  if (legacy.schema !== 1 || modern.schema !== 2 || modern.version !== require('../package.json').version ||
      current.schema !== 3 || legacy.version !== modern.version || current.version !== modern.version ||
      legacy.sha512 !== modern.sha512 || current.sha512 !== modern.sha512 || current.sha256 !== modern.sha256 ||
      legacy.size !== modern.size || current.size !== modern.size ||
      !await verifyDownloadedFile(path.join(directory, current.filename), current) ||
      readFileSync(path.join(directory, 'SHA256SUMS'), 'utf8') !== `${modern.sha256}  ${modern.filename}\n`) {
    throw new Error('Release artifacts failed verification.');
  }
  console.log(`Verified all three signed manifests and installer for ${modern.version}`);
}
main().catch((error) => { console.error(error.message); process.exitCode = 1; });
