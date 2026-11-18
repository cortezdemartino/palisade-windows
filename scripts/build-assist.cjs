'use strict';
const fs = require('node:fs/promises');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { FILENAME, verifyGuardian } = require('../src/assist/guardian.cjs');
const root = path.join(__dirname, '..');

const { nativeNotices } = require('./native-notices.cjs');

const SENSITIVE_FEATURES = Object.freeze(['native-input', 'native-resources']);
function qualificationFeatures(value = '') {
  if (typeof value !== 'string') throw Error('Invalid qualification feature selection');
  const features = value === '' ? [] : value.split(',');
  if (features.length > 2 || new Set(features).size !== features.length ||
      features.some(value => !SENSITIVE_FEATURES.includes(value))) throw Error('Invalid qualification feature selection');
  return Object.freeze([...features].sort());
}
function verifyBuildPolicy(policy, qualification, features = []) {
  if (!Array.isArray(features) || features.some(value => typeof value !== 'string')) throw Error('Invalid qualification feature selection');
  features = qualificationFeatures(features.join(','));
  if (!qualification && features.length) throw Error('Sensitive helpers require an isolated qualification build');
  const fields = ['protocol', 'version', 'origin', 'namespace', 'issuerFingerprint', 'nativeInput', 'localYield',
    'compiledNativeInput', 'compiledNativeResources'];
  if (!policy || Object.keys(policy).sort().join(',') !== fields.sort().join(',') ||
      policy.protocol !== 'palisade.assist.build' || policy.version !== 1 ||
      policy.origin !== (qualification?.origin || 'https://palisade.onyxops.org') ||
      policy.namespace !== (qualification?.namespace || null) || policy.issuerFingerprint !== (qualification?.issuerFingerprint || null) ||
      policy.nativeInput !== false || policy.localYield !== false ||
      policy.compiledNativeInput !== features.includes('native-input') ||
      policy.compiledNativeResources !== features.includes('native-resources')) throw Error('Assist compiled policy mismatch');
  return Object.freeze({ ...policy });
}
async function buildAssist(options = {}) {
  if (process.platform !== 'win32' || process.arch !== 'x64') throw new Error('Build the Assist helper on Windows x64');
  const cargo = process.env.CARGO || 'cargo';
  const { qualification } = require('../src/assist/qualification.cjs');
  if (!options || Object.keys(options).some(key => !['qualificationBuild', 'features'].includes(key)) ||
      (options.qualificationBuild !== undefined && typeof options.qualificationBuild !== 'boolean') ||
      (options.features !== undefined && (!Array.isArray(options.features) || options.features.some(value => typeof value !== 'string')))) throw Error('Invalid Assist build options');
  const features = qualificationFeatures((options.features || []).join(','));
  const qualified = options.qualificationBuild === true;
  if ((qualified && (!qualification || process.env.PALISADE_ASSIST_QUALIFICATION_BUILD !== '1')) ||
      (!qualified && (qualification || features.length))) throw Error('Sensitive helpers require an isolated qualification build');
  const env = { ...process.env };
  // Ambient feature flags never select Cargo features. The validated beforePack
  // context passes an explicit list; standalone/production builds select none.
  delete env.PALISADE_ASSIST_QUALIFICATION_FEATURES;
  for (const name of ['PALISADE_ASSIST_BUILD_ORIGIN', 'PALISADE_ASSIST_BUILD_ISSUER', 'PALISADE_ASSIST_BUILD_NAMESPACE']) delete env[name];
  if (qualification) Object.assign(env, { PALISADE_ASSIST_BUILD_ORIGIN: qualification.origin,
    PALISADE_ASSIST_BUILD_ISSUER: qualification.issuerFingerprint, PALISADE_ASSIST_BUILD_NAMESPACE: qualification.namespace });
  const result = spawnSync(cargo, ['build', '--release', '--locked', '--manifest-path', 'native/assist/Cargo.toml',
    ...(features.length ? ['--features', features.join(',')] : [])],
    { cwd: root, env, windowsHide: true, stdio: 'inherit', shell: false });
  if (result.error || result.status !== 0) throw new Error('Assist helper compilation failed');
  const output = path.join(root, 'native/assist/target/release/palisade-assist-guardian.exe');
  const information = spawnSync(output, ['--build-info'], { windowsHide: true, shell: false, encoding: 'utf8' });
  if (information.error || information.status !== 0) throw Error('Assist compiled policy unavailable');
  const policy = JSON.parse(information.stdout);
  verifyBuildPolicy(policy, qualification, features);
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
module.exports = { buildAssist, qualificationFeatures, verifyBuildPolicy };
if (require.main === module) buildAssist().catch(error => { console.error(error.message); process.exitCode = 1; });
