'use strict';
// Pure/VM build policy checks. No real Cargo, helper, app, signing or packaging.
const test = require('node:test'), assert = require('node:assert/strict'), vm = require('node:vm');
const fs = require('node:fs'), path = require('node:path');
const { createHash } = require('node:crypto');
const { qualificationFeatures, verifyBuildPolicy } = require('../scripts/build-assist.cjs');
const { selectedFeatures } = require('../scripts/build-assist-qualification.cjs');
const fixture = { origin: 'https://fixture.example:8443', issuerFingerprint: 'a'.repeat(64), namespace: 'acceptance-fixture' };
const normal = value => JSON.parse(JSON.stringify(value));
const policy = (profile = null, features = []) => ({ protocol: 'palisade.assist.build', version: 1,
  origin: profile?.origin || 'https://palisade.onyxops.org', namespace: profile?.namespace || null,
  issuerFingerprint: profile?.issuerFingerprint || null, nativeInput: false, localYield: false,
  compiledNativeInput: features.includes('native-input'), compiledNativeResources: features.includes('native-resources') });

test('qualification features require explicit closed CLI selections and preserve capture-only default', () => {
  assert.deepEqual(selectedFeatures(), []);
  assert.deepEqual(selectedFeatures(['--clipboard-text', '--native-input']), ['native-input', 'native-resources']);
  assert.deepEqual(qualificationFeatures('native-resources,native-input'), ['native-input', 'native-resources']);
  for (const args of [['--all-features'], ['--local-yield'], ['--clipboard-text', '--clipboard-text'], ['--native-input', '--execute'], [true]])
    assert.throws(() => selectedFeatures(args));
  for (const value of [true, 'native-input,native-input', 'native-input,', 'native-input --all-features', 'local-yield'])
    assert.throws(() => qualificationFeatures(value));
});

test('compiled Cargo flags are independently checked rather than inferred from non-authorizing nativeInput status', () => {
  assert.deepEqual(verifyBuildPolicy(policy(), null), policy());
  assert.deepEqual(verifyBuildPolicy(policy(fixture, ['native-input', 'native-resources']), fixture, ['native-input', 'native-resources']), policy(fixture, ['native-input', 'native-resources']));
  for (const patch of [{ compiledNativeInput: true }, { compiledNativeResources: true }, { compiledNativeResources: undefined },
    { localYield: true }, { nativeInput: true }, { issuerFingerprint: 'b'.repeat(64) }, { version: 2 }, { allowed: true }])
    assert.throws(() => verifyBuildPolicy({ ...policy(), ...patch }, null));
  assert.throws(() => verifyBuildPolicy(policy(fixture), null));
  assert.throws(() => verifyBuildPolicy(policy(fixture), fixture, ['native-resources']));
  assert.throws(() => verifyBuildPolicy(policy(null, ['native-input']), null, ['native-input']));
  assert.throws(() => verifyBuildPolicy(policy(fixture), fixture, ['--all-features']));
});

function buildHarness({ profile = null, env = {}, actualPolicy } = {}) {
  const source = fs.readFileSync(require.resolve('../scripts/build-assist.cjs'), 'utf8');
  const calls = [], writes = [], bytes = Buffer.from('synthetic helper image');
  let notices = 0, verified = 0;
  const module = { exports: {} };
  vm.runInNewContext(source, { module, __dirname: path.dirname(require.resolve('../scripts/build-assist.cjs')),
    process: { platform: 'win32', arch: 'x64', env }, require(name) {
      if (name === 'node:fs/promises') return { readFile: async () => bytes, mkdir: async () => {}, writeFile: async (...args) => { writes.push(args); } };
      if (name === 'node:child_process') return { spawnSync(binary, args, options) {
        calls.push({ binary, args, options });
        return calls.length === 1 ? { status: 0 } : { status: 0, stdout: JSON.stringify(actualPolicy || policy(profile, [])) };
      } };
      if (name === '../src/assist/qualification.cjs') return { qualification: profile };
      if (name === '../src/assist/guardian.cjs') return { FILENAME: 'Palisade-Assist.exe', verifyGuardian: async () => { verified++; } };
      if (name === './native-notices.cjs') return { nativeNotices: async () => { notices++; } };
      return require(name);
    } });
  return { build: module.exports.buildAssist, calls, writes, bytes, notices: () => notices, verified: () => verified };
}

test('standalone helper build ignores ambient qualification features and strips compile-time trust overrides', async () => {
  const h = buildHarness({ env: { PALISADE_ASSIST_QUALIFICATION_FEATURES: 'native-input,native-resources',
    PALISADE_ASSIST_BUILD_ORIGIN: 'https://other.example', PALISADE_ASSIST_BUILD_ISSUER: 'b'.repeat(64), PALISADE_ASSIST_BUILD_NAMESPACE: 'wrong' } });
  await h.build();
  assert.deepEqual(normal(h.calls[0].args), ['build', '--release', '--locked', '--manifest-path', 'native/assist/Cargo.toml']);
  for (const name of ['PALISADE_ASSIST_QUALIFICATION_FEATURES', 'PALISADE_ASSIST_BUILD_ORIGIN', 'PALISADE_ASSIST_BUILD_ISSUER', 'PALISADE_ASSIST_BUILD_NAMESPACE'])
    assert.equal(Object.hasOwn(h.calls[0].options.env, name), false);
  assert.equal(h.calls[0].options.shell, false); assert.equal(h.calls[0].options.windowsHide, true);
  assert.equal(h.verified(), 1); assert.equal(h.notices(), 1);
  const manifest = JSON.parse(h.writes.find(([file]) => file.endsWith('guardian-integrity.json'))[1]);
  assert.equal(manifest.sha256, createHash('sha256').update(h.bytes).digest('hex'));
});

test('only validated isolated build options select sensitive Cargo features and attest their actual bytes', async () => {
  const features = ['native-input', 'native-resources'];
  const h = buildHarness({ profile: fixture, env: { PALISADE_ASSIST_QUALIFICATION_BUILD: '1' }, actualPolicy: policy(fixture, features) });
  await h.build({ qualificationBuild: true, features });
  assert.deepEqual(normal(h.calls[0].args.slice(-2)), ['--features', 'native-input,native-resources']);
  assert.equal(h.calls[0].options.env.PALISADE_ASSIST_BUILD_ORIGIN, fixture.origin);
  assert.equal(h.calls[0].options.env.PALISADE_ASSIST_BUILD_ISSUER, fixture.issuerFingerprint);
  assert.equal(h.calls[0].options.env.PALISADE_ASSIST_BUILD_NAMESPACE, fixture.namespace);
  assert.equal(h.verified(), 1);
  for (const [options, profile, env] of [
    [{ features: ['native-input'] }, null, {}], [{ qualificationBuild: true, features }, null, { PALISADE_ASSIST_QUALIFICATION_BUILD: '1' }],
    [{ qualificationBuild: true, features }, fixture, {}], [{ features: ['--all-features'] }, null, {}],
    [{ features: [undefined] }, null, {}], [{ qualificationBuild: true, approved: true }, fixture, { PALISADE_ASSIST_QUALIFICATION_BUILD: '1' }],
  ]) {
    const denied = buildHarness({ profile, env }); await assert.rejects(denied.build(options));
    assert.equal(denied.calls.length, 0); assert.equal(denied.writes.length, 0);
  }
});

test('ambient or substituted compiled sensitive flags cannot pass ordinary helper packaging', async () => {
  for (const actualPolicy of [policy(null, ['native-input']), policy(null, ['native-resources']), { ...policy(), localYield: true }]) {
    const h = buildHarness({ actualPolicy }); await assert.rejects(h.build(), /compiled policy mismatch/);
    assert.equal(h.writes.length, 0); assert.equal(h.notices(), 0); assert.equal(h.verified(), 0);
  }
});

function beforePackHarness(profile, env) {
  const calls = [], module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(require.resolve('../scripts/before-pack.cjs'), 'utf8'), { module, process: { env }, require(name) {
    if (name === '../src/assist/qualification.cjs') return { qualification: profile };
    if (name === './build-assist.cjs') return { qualificationFeatures, buildAssist: async value => { calls.push(['assist', normal(value)]); } };
    if (name === './bundle-manager.cjs') return async () => { calls.push(['manager']); };
    if (name === './build-assist-preload.cjs') return { buildAssistPreload: async () => {} };
    if (name === './build-update.cjs') return { buildUpdate: async () => {} };
    if (name === './release-notes.cjs') return { loadReleaseDraft() {} };
    if (name === '../package.json') return { version: '0.0.0' };
    throw Error('Unexpected import');
  } });
  return { hook: module.exports, calls };
}
const context = () => ({ electronPlatformName: 'win32', arch: 1, targets: [], packager: {
  appInfo: { id: 'org.onyxops.palisade.qualification.' + fixture.namespace, productName: 'Palisade Assist Qualification' },
  platformSpecificBuildOptions: { signAndEditExecutable: false } } });

test('beforePack cannot bundle sensitive code as a production installer or signed qualification application', async () => {
  const env = { PALISADE_ASSIST_QUALIFICATION_BUILD: '1', PALISADE_ASSIST_QUALIFICATION_FEATURES: 'native-input,native-resources' };
  const valid = beforePackHarness(fixture, env); await valid.hook(context());
  assert.deepEqual(valid.calls.find(([type]) => type === 'assist')[1], { qualificationBuild: true, features: ['native-input', 'native-resources'] });
  for (const mutate of [c => { c.targets = [{ name: 'nsis' }]; }, c => { c.arch = 3; }, c => { c.electronPlatformName = 'linux'; },
    c => { c.packager.appInfo.id = 'org.onyxops.palisade'; }, c => { c.packager.appInfo.productName = 'Palisade'; },
    c => { c.packager.platformSpecificBuildOptions.signAndEditExecutable = true; }, c => { delete c.targets; }]) {
    const denied = beforePackHarness(fixture, env), candidate = context(); mutate(candidate);
    await assert.rejects(denied.hook(candidate)); assert.equal(denied.calls.length, 0);
  }
  for (const ambient of [env, { PALISADE_ASSIST_QUALIFICATION_FEATURES: 'native-resources' }]) {
    const denied = beforePackHarness(null, ambient); await assert.rejects(denied.hook({ packager: { appInfo: { id: 'org.onyxops.palisade' } } }));
    assert.equal(denied.calls.length, 0);
  }
});

function qualificationHarness(settings = {}) {
  const filename = require.resolve('../scripts/build-assist-qualification.cjs'), root = path.join(path.dirname(filename), '..');
  const profile = path.join(root, 'src/assist/qualification.json');
  const output = path.join(root, 'out', 'assist-qualification-' + fixture.namespace);
  const report = path.join(output, 'qualification-build.json');
  const configFile = path.join(root, 'public-fixture.json');
  const files = new Map([[configFile, JSON.stringify(fixture)]]), calls = [], writes = [], removed = [];
  const revision = 'b'.repeat(40), module = { exports: {} };
  let revisions = 0;
  if (settings.existingProfile) files.set(profile, settings.existingProfile);
  if (settings.existingReport) files.set(report, settings.existingReport);
  const missing = () => Object.assign(Error('Missing test file'), { code: 'ENOENT' });
  const fakeFs = {
    async readFile(file) { if (!files.has(file)) throw missing(); return files.get(file); },
    async writeFile(file, value, options) {
      if (options?.flag === 'wx' && files.has(file)) throw Object.assign(Error('Already exists'), { code: 'EEXIST' });
      files.set(file, value); writes.push(file);
    },
    async mkdir() {},
    async rename(from, to) { if (!files.has(from)) throw missing(); files.set(to, files.get(from)); files.delete(from); },
    async unlink(file) {
      if (file === profile && settings.cleanupFails) throw Error('Profile cleanup denied');
      files.delete(file); removed.push(file);
    },
  };
  const mockRequire = name => {
    if (name === 'node:fs/promises') return fakeFs;
    if (name === 'node:fs') return { createReadStream: async function* () { yield Buffer.from('mock packaged archive'); } };
    if (name === 'node:child_process') return { spawnSync(binary, args, options) {
      calls.push({ binary, args, options });
      if (binary === 'git') {
        if (args[0] === 'rev-parse') return { status: 0, stdout: (++revisions > 1 && settings.changedRevision ? 'c'.repeat(40) : revision) + '\n' };
        return { status: 0, stdout: settings.dirty ? ' M tracked-file\n' : '' };
      }
      if (binary === 'test-node') {
        if (settings.changedProfile) files.set(profile, 'changed concurrently');
        return { status: settings.packagingFails ? 1 : 0 };
      }
      assert.deepEqual(normal(args), ['--build-info']);
      return { status: 0, stdout: JSON.stringify(settings.actualPolicy || policy(fixture, settings.features || [])) };
    } };
    if (name === '@electron/asar') return { extractFile(archive, file) {
      return Buffer.from(file.endsWith('qualification.json') ? JSON.stringify(fixture) : 'mock integrity manifest');
    } };
    if (name === '../src/assist/guardian.cjs') return { verifyGuardian: async () => ({ sha256: 'd'.repeat(64), size: 123 }) };
    if (name === './build-assist.cjs') return { verifyBuildPolicy };
    if (name === '../package.json') return { version: '0.0.0' };
    return require(name);
  };
  mockRequire.resolve = name => { assert.equal(name, 'electron-builder/cli.js'); return 'mock-builder-cli.js'; };
  vm.runInNewContext(fs.readFileSync(filename, 'utf8'), { module, require: mockRequire, __dirname: path.dirname(filename), Buffer,
    process: { execPath: 'test-node', env: { PALISADE_ASSIST_QUALIFICATION_FEATURES: 'native-input,native-resources',
      CSC_LINK: 'must-not-forward', WIN_CSC_LINK: 'must-not-forward', CSC_KEY_PASSWORD: 'must-not-forward',
      WIN_CSC_KEY_PASSWORD: 'must-not-forward', CSC_NAME: 'must-not-forward' } } });
  return { build: options => module.exports.build(configFile, options), files, calls, writes, removed, profile, report, revision };
}

test('qualification wrapper builds only unsigned isolated directories and writes evidence after profile cleanup', async () => {
  const h = qualificationHarness({ existingReport: 'previous evidence' });
  const record = await h.build();
  const builder = h.calls.find(call => call.binary === 'test-node');
  assert.deepEqual(normal(builder.args.slice(1, 7)), ['--win', 'dir', '--x64', '--publish', 'never',
    '--config.appId=org.onyxops.palisade.qualification.' + fixture.namespace]);
  assert.ok(builder.args.includes('--config.win.signAndEditExecutable=false'));
  assert.ok(builder.args.includes('--config.forceCodeSigning=false'));
  assert.equal(builder.options.shell, false); assert.equal(builder.options.windowsHide, true);
  assert.equal(builder.options.env.PALISADE_ASSIST_QUALIFICATION_FEATURES, '');
  assert.equal(builder.options.env.CSC_IDENTITY_AUTO_DISCOVERY, 'false');
  for (const name of ['CSC_LINK', 'WIN_CSC_LINK', 'CSC_KEY_PASSWORD', 'WIN_CSC_KEY_PASSWORD', 'CSC_NAME'])
    assert.equal(Object.hasOwn(builder.options.env, name), false);
  assert.equal(h.files.has(h.profile), false); assert.ok(h.removed.includes(h.profile));
  assert.equal(record.sourceCommit, h.revision);
  assert.deepEqual(normal(record.cargoFeatures), []);
  assert.equal(record.endpointStarted, false); assert.equal(record.deviceAccepted, false); assert.equal(record.releaseApproved, false);
  assert.equal(JSON.parse(h.files.get(h.report)).archiveSha256, createHash('sha256').update('mock packaged archive').digest('hex'));
  assert.ok([...h.files].some(([file, content]) => file.includes('qualification-build.previous-') && content === 'previous evidence'));
});

test('qualification wrapper never reports success for failed builds, source drift or profile cleanup failures', async () => {
  for (const settings of [{ packagingFails: true }, { changedRevision: true }, { changedProfile: true }, { cleanupFails: true },
    { actualPolicy: policy(fixture, ['native-input']) }]) {
    const h = qualificationHarness({ ...settings, existingReport: 'previous evidence' });
    await assert.rejects(h.build());
    assert.equal(h.files.has(h.report), false);
    assert.ok([...h.files].some(([file, content]) => file.includes('qualification-build.previous-') && content === 'previous evidence'));
    if (settings.changedProfile) assert.equal(h.files.get(h.profile), 'changed concurrently');
    else if (!settings.cleanupFails) assert.equal(h.files.has(h.profile), false);
  }
  for (const settings of [{ dirty: true }, { existingProfile: 'another build profile' }]) {
    const h = qualificationHarness(settings); await assert.rejects(h.build());
    assert.equal(h.calls.some(call => call.binary === 'test-node'), false);
    assert.equal(h.files.has(h.report), false);
    if (settings.existingProfile) assert.equal(h.files.get(h.profile), settings.existingProfile);
  }
});

test('qualification wrapper records requested capabilities only after the packaged helper attests matching features', async () => {
  const h = qualificationHarness({ features: ['native-input', 'native-resources'] });
  const record = await h.build(['--native-input', '--clipboard-text']);
  assert.equal(h.calls.find(call => call.binary === 'test-node').options.env.PALISADE_ASSIST_QUALIFICATION_FEATURES, 'native-input,native-resources');
  assert.deepEqual(normal(record.requestedCapabilities), { nativeInput: true, clipboardText: true });
  assert.equal(record.nativePolicy.compiledNativeInput, true); assert.equal(record.nativePolicy.compiledNativeResources, true);
  assert.equal(record.nativePolicy.localYield, false);
});
