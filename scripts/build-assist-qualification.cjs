'use strict';
// Public trust inputs only. This builds and verifies a distinct unpacked
// candidate; it never installs or starts the application/native endpoint.
const fs = require('node:fs/promises'), path = require('node:path'), os = require('node:os');
const { createReadStream } = require('node:fs');
const { createHash, randomUUID } = require('node:crypto');
const { spawnSync } = require('node:child_process');
const { extractFile } = require('@electron/asar');
const { validateQualification } = require('../src/assist/qualification.cjs');
const { verifyGuardian } = require('../src/assist/guardian.cjs');
const { verifyBuildPolicy } = require('./build-assist.cjs');
const root = path.join(__dirname, '..');
function selectedFeatures(options = []) {
  if (!Array.isArray(options) || options.length > 2 || new Set(options).size !== options.length ||
      options.some(value => !['--native-input', '--clipboard-text'].includes(value))) throw Error('Unknown or repeated qualification build option');
  return Object.freeze([...(options.includes('--native-input') ? ['native-input'] : []),
    ...(options.includes('--clipboard-text') ? ['native-resources'] : [])]);
}
function sourceRevision() {
  const settings = { cwd: root, shell: false, windowsHide: true, encoding: 'utf8' };
  const head = spawnSync('git', ['rev-parse', 'HEAD'], settings);
  const status = spawnSync('git', ['status', '--porcelain=v1', '--untracked-files=normal'], settings);
  if (head.error || status.error || head.status !== 0 || status.status !== 0 ||
      !/^[a-f0-9]{40}\r?\n?$/.test(head.stdout) || status.stdout.trim()) throw Error('Qualification packaging requires a clean committed source revision');
  return head.stdout.trim();
}
async function hashFile(file) {
  const digest = createHash('sha256');
  for await (const bytes of createReadStream(file)) digest.update(bytes);
  return digest.digest('hex');
}
async function verifyCandidate(output, config, features) {
  const resources = path.join(output, 'win-unpacked', 'resources'), archive = path.join(resources, 'app.asar');
  const embedded = validateQualification(JSON.parse(extractFile(archive, path.normalize('src/assist/qualification.json')).toString('utf8')));
  if (JSON.stringify(embedded) !== JSON.stringify(config)) throw Error('Packaged qualification profile changed during the build');
  const manifestFile = path.join(os.tmpdir(), 'palisade-qualification-' + randomUUID() + '.json');
  await fs.writeFile(manifestFile, extractFile(archive, path.normalize('src/assist/guardian-integrity.json')), { flag: 'wx' });
  try {
    const binary = path.join(resources, 'assist', 'Palisade-Assist.exe');
    const manifest = await verifyGuardian(binary, manifestFile);
    // Fixed non-authorizing metadata mode only, never the app or --tree-endpoint.
    const information = spawnSync(binary, ['--build-info'], { shell: false, windowsHide: true, encoding: 'utf8' });
    if (information.error || information.status !== 0) throw Error('Packaged qualification build policy unavailable');
    const nativePolicy = verifyBuildPolicy(JSON.parse(information.stdout), config, features);
    return { nativePolicy, helperSha256: manifest.sha256, helperBytes: manifest.size, archiveSha256: await hashFile(archive) };
  } finally { await fs.unlink(manifestFile); }
}
async function build(configFile, options = []) {
  if (!configFile) throw Error('Provide the public disposable issuer profile JSON path');
  const features = selectedFeatures(options);
  const config = validateQualification(JSON.parse(await fs.readFile(configFile, 'utf8')));
  const revision = sourceRevision(), profile = path.join(root, 'src/assist/qualification.json');
  const profileText = JSON.stringify(config, null, 2) + '\n';
  const output = path.join(root, 'out', 'assist-qualification-' + config.namespace);
  const report = path.join(output, 'qualification-build.json');
  let record;
  // Refuse existing/concurrent profiles; never overwrite another build's trust.
  await fs.writeFile(profile, profileText, { flag: 'wx' });
  try {
    await fs.mkdir(output, { recursive: true });
    try { await fs.rename(report, path.join(output, 'qualification-build.previous-' + randomUUID() + '.json')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    const builder = require.resolve('electron-builder/cli.js');
    const env = { ...process.env, PALISADE_ASSIST_QUALIFICATION_BUILD: '1',
      PALISADE_ASSIST_QUALIFICATION_FEATURES: features.join(','), CSC_IDENTITY_AUTO_DISCOVERY: 'false' };
    for (const name of ['CSC_LINK', 'WIN_CSC_LINK', 'CSC_KEY_PASSWORD', 'WIN_CSC_KEY_PASSWORD', 'CSC_NAME']) delete env[name];
    const result = spawnSync(process.execPath, [builder, '--win', 'dir', '--x64', '--publish', 'never',
      '--config.appId=org.onyxops.palisade.qualification.' + config.namespace,
      '--config.productName=Palisade Assist Qualification', '--config.executableName=Palisade',
      '--config.forceCodeSigning=false', '--config.win.signAndEditExecutable=false',
      '--config.directories.output=out/assist-qualification-' + config.namespace],
    { cwd: root, shell: false, windowsHide: true, stdio: 'inherit', env });
    if (result.error || result.status !== 0) throw Error('Qualification packaging failed');
    const verified = await verifyCandidate(output, config, features);
    if (sourceRevision() !== revision) throw Error('Source changed during qualification packaging');
    record = { format: 'palisade.assist.qualification-build', version: 1, sourceCommit: revision,
      applicationVersion: require('../package.json').version, profile: config,
      requestedCapabilities: { nativeInput: features.includes('native-input'), clipboardText: features.includes('native-resources') },
      cargoFeatures: features, ...verified, endpointStarted: false, deviceAccepted: false, releaseApproved: false };
  } finally {
    // Preserve a profile someone changed while this build was running.
    const current = await fs.readFile(profile, 'utf8').catch(error => { if (error.code === 'ENOENT') return null; throw error; });
    if (current !== null && current !== profileText) throw Error('Qualification profile changed; preserved for review');
    if (current !== null) await fs.unlink(profile);
  }
  // A failed build or profile cleanup must never leave current success evidence.
  if (sourceRevision() !== revision) throw Error('Source changed during qualification packaging');
  await fs.writeFile(report, JSON.stringify(record, null, 2) + '\n', { flag: 'wx' });
  return record;
}
module.exports = { build, selectedFeatures, verifyCandidate };
if (require.main === module) build(process.argv[2], process.argv.slice(3)).catch(error => { console.error(error.message); process.exitCode = 1; });
