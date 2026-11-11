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
  const { qualification } = require('../src/assist/qualification.cjs');
  const env = { ...process.env };
  for (const name of ['PALISADE_ASSIST_BUILD_ORIGIN', 'PALISADE_ASSIST_BUILD_ISSUER', 'PALISADE_ASSIST_BUILD_NAMESPACE']) delete env[name];
  if (qualification) Object.assign(env, { PALISADE_ASSIST_BUILD_ORIGIN: qualification.origin,
    PALISADE_ASSIST_BUILD_ISSUER: qualification.issuerFingerprint, PALISADE_ASSIST_BUILD_NAMESPACE: qualification.namespace });
  const result = spawnSync(cargo, ['build', '--release', '--locked', '--manifest-path', 'native/assist/Cargo.toml'],
    { cwd: root, env, windowsHide: true, stdio: 'inherit', shell: false });
  if (result.error || result.status !== 0) throw new Error('Assist helper compilation failed');
  const output = path.join(root, 'native/assist/target/release/palisade-assist-guardian.exe');
  const information = spawnSync(output, ['--build-info'], { windowsHide: true, shell: false, encoding: 'utf8' });
  if (information.error || information.status !== 0) throw Error('Assist compiled policy unavailable');
  const policy = JSON.parse(information.stdout);
  if (policy.origin !== (qualification?.origin || 'https://palisade.onyxops.org') ||
      policy.namespace !== (qualification?.namespace || null) || policy.issuerFingerprint !== (qualification?.issuerFingerprint || null) ||
      policy.nativeInput !== false || policy.localYield !== false) throw Error('Assist compiled policy mismatch');
  const bytes = await fs.readFile(output);
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
