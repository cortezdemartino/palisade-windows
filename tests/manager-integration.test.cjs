'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { spawnSync } = require('node:child_process');
const { generateKeyPairSync } = require('node:crypto');
const { integrate } = require('../scripts/integrate-manager.cjs');
const root = path.join(__dirname, '..');
const kit = path.join(root, 'integrations/manager');
async function candidate(t) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'manager-integration-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const manifest = JSON.parse(await fs.readFile(path.join(kit, 'manifest.json'), 'utf8'));
  const overrides = JSON.parse(await fs.readFile(path.join(kit, 'overrides.json'), 'utf8'));
  for (const [entries, sourceRoot] of [[manifest.shared, root], [overrides, path.join(kit, 'overrides')]])
    for (const entry of entries) {
      const destination = path.join(directory, entry.path);
      await fs.mkdir(path.dirname(destination), { recursive: true });
      await fs.copyFile(path.join(sourceRoot, entry.path), destination);
    }
  await fs.copyFile(path.join(root, 'assets/update-public-key.pem'), path.join(directory, 'assets/update-public-key.pem'));
  return directory;
}
test('Manager source integration is exact, repeatable and runs its shared signature/online/offline regressions', async (t) => {
  const directory = await candidate(t);
  assert.deepEqual(await integrate(directory), { changed: 0, applied: false });
  assert.deepEqual(await integrate(directory, true), { changed: 0, applied: true });
  const env = { ...process.env }; delete env.NODE_TEST_CONTEXT;
  const result = spawnSync(process.execPath, ['--test', 'tests/release-notes.test.cjs', 'tests/u1-service.test.cjs'],
    { cwd: directory, env, encoding: 'utf8', windowsHide: true, timeout: 30000 });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /# fail 0/);
});
test('Manager source integration refuses concurrent changes before writing missing source', async (t) => {
  const directory = await candidate(t);
  const target = path.join(directory, 'src/updates/notes.cjs');
  await fs.rm(target);
  const service = path.join(directory, 'src/updates/service.cjs');
  const original = await fs.readFile(service);
  await fs.appendFile(service, '\n// Concurrent maintainer change.\n');
  await assert.rejects(integrate(directory, true), /concurrent changes/);
  await assert.rejects(fs.access(target), { code: 'ENOENT' });
  await fs.writeFile(service, original);
  assert.deepEqual(await integrate(directory, true), { changed: 1, applied: true });
});
test('Manager source integration never replaces an unfamiliar trust root', async (t) => {
  const directory = await candidate(t);
  await fs.writeFile(path.join(directory, 'assets/update-public-key.pem'),
    generateKeyPairSync('ed25519').publicKey.export({ type: 'spki', format: 'pem' }));
  await assert.rejects(integrate(directory, true), /trust anchor differs/);
});
