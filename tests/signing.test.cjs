'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { generateKeyPairSync } = require('node:crypto');
const { spawnSync } = require('node:child_process');

async function fixture(run) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'palisade-signing-test-'));
  const secrets = await fs.mkdtemp(path.join(os.tmpdir(), 'palisade-test-keys-'));
  try {
    for (const folder of ['scripts', 'src/updates', 'assets', 'out']) {
      await fs.mkdir(path.join(directory, folder), { recursive: true });
    }
    for (const file of ['scripts/sign-release.cjs', 'scripts/verify-release.cjs', 'scripts/signing-key.cjs',
      'scripts/authorize-update-key.cjs', 'src/updates/key-trust.cjs', 'src/updates/release.cjs']) {
      await fs.copyFile(path.join(__dirname, '..', file), path.join(directory, file));
    }
    await fs.writeFile(path.join(directory, 'package.json'), JSON.stringify({ version: '0.2.2' }));
    const root = generateKeyPairSync('ed25519');
    const rootFile = path.join(secrets, 'original.pem');
    await fs.writeFile(rootFile, root.privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
    await fs.writeFile(path.join(directory, 'assets/update-public-key.pem'), root.publicKey.export({ type: 'spki', format: 'pem' }));
    await fs.writeFile(path.join(directory, 'assets/update-key-transitions.json'), '[]');
    const installer = path.join(directory, 'out/Palisade-Setup-0.2.2-x64.exe');
    await fs.writeFile(installer, 'fixture installer');
    const invoke = (script, args = [], primary = rootFile, legacy = '') => spawnSync(process.execPath,
      [path.join(directory, 'scripts', script), ...args], { cwd: directory, encoding: 'utf8',
        env: { ...process.env, PALISADE_UPDATE_PRIVATE_KEY_FILE: primary, PALISADE_UPDATE_LEGACY_PRIVATE_KEY_FILE: legacy } });
    await run({ directory, secrets, installer, invoke, rootFile });
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
    await fs.rm(secrets, { recursive: true, force: true });
  }
}

test('release CLI verifies all formats; missing credentials, in-repository keys and tampered artifacts fail closed', async () => {
  await fixture(async ({ directory, installer, invoke }) => {
    assert.notEqual(invoke('sign-release.cjs', [installer, 'out'], '').status, 0);
    const inside = invoke('sign-release.cjs', [installer, 'out'], path.join(directory, 'package.json'));
    assert.notEqual(inside.status, 0);
    assert.match(inside.stderr, /outside the source repository/);
    const signed = invoke('sign-release.cjs', [installer, 'out']);
    assert.equal(signed.status, 0, signed.stderr);
    const verified = invoke('verify-release.cjs', ['out']);
    assert.equal(verified.status, 0, verified.stderr);
    await fs.appendFile(installer, 'tampering');
    assert.notEqual(invoke('verify-release.cjs', ['out']).status, 0);
    await fs.writeFile(path.join(directory, 'assets/update-public-key.pem'), generateKeyPairSync('ed25519').publicKey.export({ type: 'spki', format: 'pem' }));
    assert.notEqual(invoke('sign-release.cjs', [installer, 'out']).status, 0);
  });
});

test('rotation CLI requires predecessor authorization and retains original-key manifests for old clients', async () => {
  await fixture(async ({ directory, secrets, installer, invoke, rootFile }) => {
    const successor = generateKeyPairSync('ed25519');
    const privateFile = path.join(secrets, 'successor-private.pem');
    const publicFile = path.join(directory, 'assets/successor-public.pem');
    await fs.writeFile(privateFile, successor.privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
    await fs.writeFile(publicFile, successor.publicKey.export({ type: 'spki', format: 'pem' }));
    const chainFile = path.join(directory, 'authorized-chain.json');
    assert.notEqual(invoke('sign-release.cjs', [installer, 'out'], privateFile).status, 0);
    assert.notEqual(invoke('authorize-update-key.cjs', [publicFile, '0.2.2', chainFile], privateFile).status, 0);
    await assert.rejects(fs.readFile(chainFile));
    const authorized = invoke('authorize-update-key.cjs', [publicFile, '0.2.2', chainFile]);
    assert.equal(authorized.status, 0, authorized.stderr);
    assert.notEqual(invoke('authorize-update-key.cjs', [publicFile, '0.2.2', chainFile]).status, 0);
    await fs.copyFile(chainFile, path.join(directory, 'assets/update-key-transitions.json'));
    const missingLegacy = invoke('sign-release.cjs', [installer, 'out'], privateFile);
    assert.notEqual(missingLegacy.status, 0);
    assert.match(missingLegacy.stderr, /original private key/);
    assert.deepEqual(await fs.readdir(path.join(directory, 'out')), [path.basename(installer)]);
    const signed = invoke('sign-release.cjs', [installer, 'out'], privateFile, rootFile);
    assert.equal(signed.status, 0, signed.stderr);
    const verified = invoke('verify-release.cjs', ['out']);
    assert.equal(verified.status, 0, verified.stderr);
    const envelope = JSON.parse(await fs.readFile(path.join(directory, 'out/release-v3.json')));
    envelope.transitions[0].payload.minVersion = '0.2.1';
    await fs.writeFile(path.join(directory, 'out/release-v3.json'), JSON.stringify(envelope));
    assert.notEqual(invoke('verify-release.cjs', ['out']).status, 0);
  });
});
