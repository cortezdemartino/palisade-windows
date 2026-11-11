'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { nativeNotices } = require('./native-notices.cjs');
const { verifyHelper, FILENAME, PROTOCOL } = require('../src/updates/native-helper.cjs');
const root = path.join(__dirname, '..');
async function buildUpdate() {
  if (process.platform !== 'win32' || process.arch !== 'x64') throw Error('Build the update helper on Windows x64.');
  const cargo = process.env.CARGO || 'cargo';
  const result = spawnSync(cargo, ['build', '--release', '--locked', '--manifest-path', 'native/update/Cargo.toml'],
    { cwd: root, windowsHide: true, stdio: 'inherit', shell: false });
  if (result.error || result.status !== 0) throw Error('Native updater compilation failed.');
  const bytes = await fs.readFile(path.join(root, 'native/update/target/release/palisade-update-helper.exe'));
  const manifest = { protocol: PROTOCOL, schema: 1, version: require('../package.json').version,
    filename: FILENAME, size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
  const destination = path.join(root, '.update');
  await fs.mkdir(destination, { recursive: true });
  await nativeNotices({ cargo, manifest: 'native/update/Cargo.toml', destination,
    title: 'Palisade native updater third-party notices', ownPackage: 'palisade-update-helper' });
  await fs.writeFile(path.join(destination, FILENAME), bytes);
  const manifestFile = path.join(root, 'src/updates/helper-integrity.json');
  await fs.writeFile(manifestFile, JSON.stringify(manifest, null, 2) + '\n');
  await verifyHelper(path.join(destination, FILENAME), manifestFile, manifest.version);
}
module.exports = { buildUpdate };
if (require.main === module) buildUpdate().catch(error => { console.error(error.message); process.exitCode = 1; });
