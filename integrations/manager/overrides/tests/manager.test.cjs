'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { generateKeyPairSync, sign, createHash } = require('node:crypto');
const { Settings, defaults, validate } = require('../src/config.cjs');
const { validatePath, prepareDestination, inspectInstallation, runInstaller } = require('../src/windows.cjs');
const { download, offline, enforceVersion, discover } = require('../src/updates/service.cjs');
const { signedBytes, RELEASES_API, releaseAssetUrl } = require('../src/updates/release.cjs');
const content = Buffer.from('a verified package');
const payload = { schema: 1, channel: 'stable', version: '1.2.3', filename: 'Palisade-Setup-1.2.3-x64.exe',
  sha512: createHash('sha512').update(content).digest('base64'), size: content.length, publishedAt: '2026-10-15T00:00:00.000Z' };
const pair = generateKeyPairSync('ed25519');
const key = pair.publicKey.export({ type: 'spki', format: 'pem' });
const envelope = { payload, signature: sign(null, signedBytes(payload), pair.privateKey).toString('base64') };
async function temp(t) { const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'palisade-test-')); t.after(() => fs.rm(dir, { recursive: true, force: true })); return dir; }
function response(bytes) { return { ok: true, status: 200, headers: new Map(), body: (async function* () { yield Buffer.isBuffer(bytes) ? bytes : Buffer.from(JSON.stringify(bytes)); })() }; }
test('settings persist validated preferences and identify malformed configuration', async t => {
  const dir = await temp(t); const settings = new Settings(dir);
  await settings.load(); assert.deepEqual(settings.value, defaults());
  settings.value.theme = 'light'; settings.record('1.2.3', 'completed'); await settings.save();
  const reopened = new Settings(dir); await reopened.load();
  assert.equal(reopened.value.theme, 'light'); assert.equal(reopened.value.history[0].version, '1.2.3');
  await fs.writeFile(settings.file, '{bad'); await reopened.load(); assert.ok(reopened.warning);
});
test('configuration rejects invalid fields', () => {
  for (const delta of [{ theme: 'remote' }, { desktopShortcut: 'yes' }, { highestVersion: '1..3' },
    { history: [{ version: '1.2.3', result: 'arbitrary', at: 'now' }] },
    { pending: { path: '../somewhere', version: '1.2.3' } }]) assert.throws(() => validate({ ...defaults(), ...delta }));
});
test('version policy permits repair and upgrade but blocks downgrade and wrong architecture', () => {
  enforceVersion(payload, '1.2.3', '1.2.3', 'x64');
  enforceVersion(payload, '1.2.2', '1.0.0', 'x64');
  assert.throws(() => enforceVersion(payload, '1.2.4', '0.0.0', 'x64'), /older/);
  assert.throws(() => enforceVersion(payload, null, '2.0.0', 'x64'), /older/);
  assert.throws(() => enforceVersion(payload, null, '0.0.0', 'arm64'), /x64/);
});
test('unsafe installation directories are rejected', () => {
  const env = { SystemRoot: 'C:\\Windows', APPDATA: 'C:\\Users\\Test\\AppData\\Roaming', USERPROFILE: 'C:\\Users\\Test', ProgramFiles: 'C:\\Program Files' };
  for (const value of ['C:\\', '\\\\server\\app', 'relative', 'C:\\Windows\\Palisade', 'C:\\Users\\Test', 'C:\\Users\\Test\\AppData\\Roaming\\Palisade', 'C:\\Apps\\NUL', 'C:\\Apps\\bad.', 'C:\\Apps\\bad:ads']) assert.throws(() => validatePath(value, env), value);
  assert.equal(validatePath('C:\\Users\\Test\\AppData\\Local\\Programs\\Palisade', env), 'C:\\Users\\Test\\AppData\\Local\\Programs\\Palisade');
});
test('disk space failure occurs before creating the installation folder', async t => {
  const dir = await temp(t); const target = path.join(dir, 'Palisade');
  await assert.rejects(prepareDestination(target, 1000, async () => ({ bavail: 1, bsize: 1 })), /Not enough/);
  await assert.rejects(fs.access(target));
});
test('missing files distinguish fresh from incomplete registered installations', async t => {
  const dir = await temp(t);
  assert.equal((await inspectInstallation(dir)).status, 'missing');
  const found = await inspectInstallation(dir, { version: '1.2.3' });
  assert.equal(found.status, 'incomplete'); assert.equal(found.version, '1.2.3');
});
test('installer launch failure produces an actionable error', async t => {
  const dir = await temp(t);
  await assert.rejects(runInstaller(path.join(dir, 'missing.exe'), dir, false), /could not start/);
});
test('offline verification requires signature and complete package', async t => {
  const dir = await temp(t);
  await fs.writeFile(path.join(dir, 'release.json'), JSON.stringify(envelope));
  await fs.writeFile(path.join(dir, payload.filename), content);
  assert.equal((await offline(dir, key)).manifest.version, payload.version);
  await fs.writeFile(path.join(dir, payload.filename), 'altered');
  await assert.rejects(offline(dir, key), /failed verification/);
  await fs.writeFile(path.join(dir, 'release.json'), JSON.stringify({ ...envelope, signature: 'A'.repeat(86) + '==' }));
  await assert.rejects(offline(dir, key), /signature verification/);
});
test('download verifies bytes and reports progress', async t => {
  const dir = await temp(t); let progress;
  const file = await download(payload, dir, new AbortController().signal, (n, total) => { progress = [n, total]; }, async () => response(content));
  assert.deepEqual(await fs.readFile(file), content); assert.deepEqual(progress, [content.length, content.length]);
});
test('truncated, oversized and altered packages are removed', async t => {
  const dir = await temp(t);
  for (const bytes of [content.subarray(0, 3), Buffer.alloc(content.length, 1), Buffer.alloc(content.length + 1)]) {
    await assert.rejects(download(payload, dir, new AbortController().signal, () => {}, async () => response(bytes)));
    await assert.rejects(fs.access(path.join(dir, payload.filename)));
  }
});
test('interrupted download is removed and retry succeeds', async t => {
  const dir = await temp(t); const controller = new AbortController();
  const fetchFn = async () => ({ ok: true, status: 200, body: (async function* () { yield content.subarray(0, 3); controller.abort(); yield content.subarray(3); })() });
  await assert.rejects(download(payload, dir, controller.signal, () => {}, fetchFn));
  await assert.rejects(fs.access(path.join(dir, payload.filename)));
  await download(payload, dir, new AbortController().signal, () => {}, async () => response(content));
});
test('exclusive creation never overwrites or removes an existing file', async t => {
  const dir = await temp(t); const file = path.join(dir, payload.filename);
  await fs.writeFile(file, 'existing');
  await assert.rejects(download(payload, dir, new AbortController().signal, () => {}, async () => response(content)));
  assert.equal(await fs.readFile(file, 'utf8'), 'existing');
});
test('legacy discovery binds metadata to the tag and never promotes unsigned release prose', async () => {
  const signal = new AbortController().signal;
  const metadata = { draft: false, prerelease: false, tag_name: 'v1.2.3', body: '<script>untrusted</script>' };
  const found = await discover(key, signal, async url => {
    if (url === RELEASES_API) return response(metadata);
    if (url.endsWith('/release-v3.json')) return { ok: false, status: 404 };
    assert.equal(url, releaseAssetUrl('1.2.3', 'release.json')); return response(envelope);
  });
  assert.equal(found.manifest.version, '1.2.3'); assert.equal(found.notes, 'Verified release notes are unavailable.');
  metadata.tag_name = 'v1.2.4';
  await assert.rejects(discover(key, signal, async url => url === RELEASES_API ? response(metadata) :
    url.endsWith('/release-v3.json') ? { ok: false, status: 404 } : response(envelope)), /does not match/);
});
test('no published release produces an explicit error rather than an up-to-date result', async () => {
  await assert.rejects(discover(key, new AbortController().signal, async () => ({ status: 404 })), /No stable Palisade release/);
});
test('transient release-service failures retry with a bounded attempt count', async () => {
  let attempts = 0;
  await assert.rejects(discover(key, new AbortController().signal, async () => {
    attempts++; return { status: 503 };
  }), /temporarily unavailable/);
  assert.equal(attempts, 3);
});
test('cancelled discovery makes no network requests', async () => {
  const controller = new AbortController(); controller.abort(); let calls = 0;
  await assert.rejects(discover(key, controller.signal, async () => { calls++; }));
  assert.equal(calls, 0);
});
test('configuration allowlist excludes unexpected history and pending fields', () => {
  const value = validate({ ...defaults(), history: [{ version: '1.2.3', result: 'started', at: '2026-10-15T00:00:00.000Z', token: 'excluded' }],
    pending: { version: '1.2.3', path: 'C:\\Apps\\Palisade', token: 'excluded' } });
  assert.equal(value.history[0].token, undefined); assert.equal(value.pending.token, undefined);
});


test('manager runs the verified engine silently with destination and scope, without auto-launch', async () => {
  const { EventEmitter } = require('node:events');
  for (const machine of [true, false]) {
    let captured;
    await runInstaller('C:\\Stage\\setup.exe', 'C:\\Apps\\Palisade With Spaces', false, machine, {
      spawn: (file, args, options) => {
        captured = { file, args, options };
        const child = new EventEmitter();
        process.nextTick(() => child.emit('exit', 0));
        return child;
      }
    });
    assert.deepEqual(captured.args, ['/S', machine ? '/allusers' : '/currentuser', '--no-desktop-shortcut', '/D=C:\\Apps\\Palisade With Spaces']);
    assert.equal(captured.options.shell, false);
    assert.equal(captured.options.windowsHide, true);
    assert.ok(!captured.args.includes('--force-run'));
  }
});
