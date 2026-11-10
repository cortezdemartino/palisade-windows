'use strict';

const fs = require('node:fs/promises');
const path = require('node:path');
const { createHash, createPublicKey } = require('node:crypto');
const root = path.join(__dirname, '..');
const kit = path.join(root, 'integrations/manager');
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
function inside(directory, relative) {
  if (typeof relative !== 'string' || relative.includes('\\') || relative.split('/').some((part) => !part || part === '..' || part === '.'))
    throw Error('Invalid Manager integration path.');
  const result = path.resolve(directory, relative);
  if (path.relative(directory, result).startsWith('..') || path.isAbsolute(path.relative(directory, result)))
    throw Error('Manager integration path is outside its source root.');
  return result;
}
async function integrate(directory, apply = false) {
  directory = path.resolve(directory);
  const manifest = JSON.parse(await fs.readFile(path.join(kit, 'manifest.json'), 'utf8'));
  const overrides = JSON.parse(await fs.readFile(path.join(kit, 'overrides.json'), 'utf8'));
  const pkg = JSON.parse(await fs.readFile(path.join(directory, 'package.json'), 'utf8'));
  if (manifest.schema !== 1 || pkg.name !== manifest.package || pkg.version !== manifest.sourceVersion)
    throw Error('Unexpected Manager source identity/version.');
  const [desktopKey, managerKey] = await Promise.all([
    fs.readFile(path.join(root, 'assets/update-public-key.pem')), fs.readFile(path.join(directory, 'assets/update-public-key.pem'))]);
  if (!createPublicKey(desktopKey).equals(createPublicKey(managerKey))) throw Error('Manager trust anchor differs; migration review required.');
  const copies = [];
  const seen = new Set();
  for (const [entries, sourceRoot] of [[manifest.shared, root], [overrides, path.join(kit, 'overrides')]]) {
    for (const entry of entries) {
      if (seen.has(entry.path)) throw Error('Duplicate Manager integration path.');
      seen.add(entry.path);
      const bytes = await fs.readFile(inside(sourceRoot, entry.path));
      if (digest(bytes) !== entry.sha256) throw Error('Manager integration input drift: ' + entry.path);
      const destination = inside(directory, entry.path);
      let current = null;
      try { current = digest(await fs.readFile(destination)); } catch (error) { if (error.code !== 'ENOENT') throw error; }
      if (current !== entry.sha256 && current !== entry.baseSha256)
        throw Error('Manager source has concurrent changes: ' + entry.path);
      if (current !== entry.sha256) copies.push({ bytes, destination });
    }
  }
  // All source identities and content hashes are checked before the first write.
  if (apply) for (const { bytes, destination } of copies) {
    await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.writeFile(destination, bytes);
  }
  return { changed: copies.length, applied: apply };
}
if (require.main === module) {
  if (!process.argv[2] || process.argv.slice(3).some((arg) => arg !== '--apply')) {
    console.error('Usage: node scripts/integrate-manager.cjs <Manager-source-directory> [--apply]'); process.exitCode = 1;
  } else integrate(process.argv[2], process.argv.includes('--apply')).then((result) =>
    console.log(`${result.applied ? 'Applied' : 'Validated'} Manager integration: ${result.changed} file changes.`))
    .catch((error) => { console.error(error.message); process.exitCode = 1; });
}
module.exports = { integrate };
