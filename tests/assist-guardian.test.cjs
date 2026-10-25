'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { createHash } = require('node:crypto');
const { verifyGuardian, packagedGuardianPaths } = require('../src/assist/guardian.cjs');

test('guardian integrity refuses changed binary, size, filename and unsafe manifest', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'palisade-guardian-integrity-'));
  try {
    const binary = path.join(directory, 'Palisade-Assist.exe'), file = path.join(directory, 'manifest.json');
    const bytes = Buffer.from('non-executable-integrity-fixture');
    const good = { protocol: 'palisade.assist.guardian', version: 1, filename: 'Palisade-Assist.exe',
      size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
    await fs.writeFile(binary, bytes); await fs.writeFile(file, JSON.stringify(good));
    await verifyGuardian(binary, file);
    for (const bad of [{ ...good, sha256: '0'.repeat(64) }, { ...good, size: 1 },
      { ...good, filename: '../Palisade.exe' }, { ...good, version: 2 }, { ...good, command: 'run' }]) {
      await fs.writeFile(file, JSON.stringify(bad)); await assert.rejects(verifyGuardian(binary, file));
    }
    await fs.writeFile(file, JSON.stringify(good)); await fs.writeFile(binary, Buffer.alloc(bytes.length));
    await assert.rejects(verifyGuardian(binary, file), /integrity/);
    const paths = packagedGuardianPaths('C:\\fixture\\resources');
    assert.match(paths.manifestFile, /app\.asar/); assert.match(paths.binary, /Palisade-Assist\.exe$/);
  } finally { await fs.rm(directory, { recursive: true, force: true }); }
});
