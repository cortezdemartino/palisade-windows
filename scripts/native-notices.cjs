'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const root = path.join(__dirname, '..');

async function nativeNotices({ cargo, manifest, destination, title, ownPackage }) {
  const result = spawnSync(cargo, ['metadata', '--locked', '--format-version', '1', '--manifest-path', manifest],
    { cwd: root, windowsHide: true, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024, shell: false });
  if (result.error || result.status !== 0) throw new Error('Native dependency metadata unavailable');
  const sections = [title + '\nSee Cargo.lock for the locked build inputs.\n'];
  for (const item of JSON.parse(result.stdout).packages) {
    if (item.name === ownPackage) continue;
    const directory = path.dirname(item.manifest_path);
    const names = (await fs.readdir(directory)).filter(name => /^(?:licen[cs]e|copying|copyright)/i.test(name)).sort();
    if (!names.length) throw new Error('Missing license for native dependency ' + item.name);
    sections.push('\n' + item.name + ' ' + item.version + '\nLicense: ' + item.license + '\n' + (item.repository || '') + '\n');
    for (const name of names) {
      if (!(await fs.stat(path.join(directory, name))).isFile()) continue;
      sections.push('\n' + name + '\n' + await fs.readFile(path.join(directory, name), 'utf8'));
    }
  }
  await fs.writeFile(path.join(destination, 'THIRD-PARTY-NOTICES.txt'), sections.join('\n'));
  const rustc = spawnSync('rustc', ['--print', 'sysroot'], { cwd: root, encoding: 'utf8', windowsHide: true, shell: false });
  if (rustc.error || rustc.status !== 0) throw new Error('Rust runtime license metadata unavailable');
  const runtime = path.join(rustc.stdout.trim(), 'share/doc/rust');
  await fs.copyFile(path.join(runtime, 'COPYRIGHT-library.html'), path.join(destination, 'RUST-COPYRIGHT.html'));
  await fs.cp(path.join(runtime, 'licenses'), path.join(destination, 'licenses'), { recursive: true });
}

module.exports = { nativeNotices };
