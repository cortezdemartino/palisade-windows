'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { FILENAME, verifyGuardian } = require('../src/assist/guardian.cjs');
const root = path.join(__dirname, '..');

async function licenses(cargo) {
  const result = spawnSync(cargo, ['metadata', '--locked', '--format-version', '1', '--manifest-path', 'native/assist/Cargo.toml'],
    { cwd: root, windowsHide: true, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024, shell: false });
  if (result.error || result.status !== 0) throw new Error('Assist dependency metadata unavailable');
  const sections = ['Palisade Assist guardian third-party notices\nSee Cargo.lock for the locked build inputs.\n'];
  for (const item of JSON.parse(result.stdout).packages) {
    if (item.name === 'palisade-assist-guardian') continue;
    const directory = path.dirname(item.manifest_path);
    const names = (await fs.readdir(directory)).filter(name => /^(?:licen[cs]e|copying|copyright)/i.test(name)).sort();
    if (!names.length) throw new Error('Missing license for Assist dependency ' + item.name);
    sections.push('\n' + item.name + ' ' + item.version + '\nLicense: ' + item.license + '\n' + (item.repository || '') + '\n');
    for (const name of names) {
      if (!(await fs.stat(path.join(directory, name))).isFile()) continue;
      sections.push('\n' + name + '\n' + await fs.readFile(path.join(directory, name), 'utf8'));
    }
  }
  await fs.writeFile(path.join(root, '.assist/THIRD-PARTY-NOTICES.txt'), sections.join('\n'));
  const rustc = spawnSync('rustc', ['--print', 'sysroot'], { cwd: root, encoding: 'utf8', windowsHide: true, shell: false });
  if (rustc.error || rustc.status !== 0) throw new Error('Rust runtime license metadata unavailable');
  const runtime = path.join(rustc.stdout.trim(), 'share/doc/rust');
  await fs.copyFile(path.join(runtime, 'COPYRIGHT-library.html'), path.join(root, '.assist/RUST-COPYRIGHT.html'));
  await fs.cp(path.join(runtime, 'licenses'), path.join(root, '.assist/licenses'), { recursive: true });
}

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
  await licenses(cargo);
  await fs.writeFile(path.join(root, '.assist', FILENAME), bytes);
  const manifestFile = path.join(root, 'src/assist/guardian-integrity.json');
  await fs.writeFile(manifestFile, JSON.stringify(manifest, null, 2) + '\n');
  await verifyGuardian(path.join(root, '.assist', FILENAME), manifestFile);
}
module.exports = { buildAssist };
if (require.main === module) buildAssist().catch(error => { console.error(error.message); process.exitCode = 1; });
