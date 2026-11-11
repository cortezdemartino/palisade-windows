'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { FILENAME, verifyGuardian } = require('../src/assist/guardian.cjs');
const root = path.join(__dirname, '..');

const { nativeNotices } = require('./native-notices.cjs');

async function buildAssist() {
  if (process.platform !== 'win32' || process.arch !== 'x64') throw new Error('Build the Assist helper on Windows x64');
  const cargo = process.env.CARGO || 'cargo';
  const result = spawnSync(cargo, ['build', '--release', '--locked', '--manifest-path', 'native/assist/Cargo.toml'],
    { cwd: root, windowsHide: true, stdio: 'inherit', shell: false });
  if (result.error || result.status !== 0) throw new Error('Assist helper compilation failed');
  const bytes = await fs.readFile(path.join(root, 'native/assist/target/release/palisade-assist-guardian.exe'));
  // Package manifest is inside ASAR, protected by the application's existing integrity fuse.
  const manifest = { protocol: 'palisade.assist.guardian', version: 1, filename: FILENAME,
    size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
  await fs.mkdir(path.join(root, '.assist'), { recursive: true });
  await nativeNotices({ cargo, manifest: 'native/assist/Cargo.toml', destination: path.join(root, '.assist'), title: 'Palisade Assist guardian third-party notices', ownPackage: 'palisade-assist-guardian' });
  await fs.writeFile(path.join(root, '.assist', FILENAME), bytes);
  const manifestFile = path.join(root, 'src/assist/guardian-integrity.json');
  await fs.writeFile(manifestFile, JSON.stringify(manifest, null, 2) + '\n');
  await verifyGuardian(path.join(root, '.assist', FILENAME), manifestFile);
}
module.exports = { buildAssist };
if (require.main === module) buildAssist().catch(error => { console.error(error.message); process.exitCode = 1; });
