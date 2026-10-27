'use strict';
// Compile sensitive adapters, then exercise only non-authorizing probe modes.
const fs = require('node:fs/promises');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { spawnSync } = require('node:child_process');
async function main() {
  const root = path.join(__dirname, '..');
  const build = spawnSync(
    'cargo',
    ['build', '--locked', '--manifest-path', 'native/assist/Cargo.toml', '--all-features'],
    { cwd: root, windowsHide: true, stdio: 'inherit', shell: false },
  );
  if (build.error || build.status !== 0) throw Error('Feature compilation failed');
  const binary = path.join(root, 'native/assist/target/debug/palisade-assist-guardian.exe');
  const bytes = await fs.readFile(binary),
    directory = path.join(root, 'out/assist-feature-probe');
  await fs.mkdir(directory, { recursive: true });
  const manifestFile = path.join(directory, 'integrity.json');
  await fs.writeFile(
    manifestFile,
    JSON.stringify({
      protocol: 'palisade.assist.guardian',
      version: 1,
      filename: 'Palisade-Assist.exe',
      size: bytes.length,
      sha256: createHash('sha256').update(bytes).digest('hex'),
    }),
  );
  const tests = spawnSync(
    process.execPath,
    ['--test', 'tests/assist-guardian-process.native.cjs'],
    {
      cwd: root,
      windowsHide: true,
      stdio: 'inherit',
      shell: false,
      env: {
        ...process.env,
        PALISADE_ASSIST_TEST_BINARY: binary,
        PALISADE_ASSIST_TEST_MANIFEST: manifestFile,
      },
    },
  );
  if (tests.error || tests.status !== 0) throw Error('Compiled-feature probe tests failed');
  const harness = spawnSync(
    'cargo',
    [
      'test',
      '--no-run',
      '--locked',
      '--manifest-path',
      'native/assist/Cargo.toml',
      '--all-features',
      '--message-format=json',
    ],
    { cwd: root, windowsHide: true, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024, shell: false },
  );
  if (harness.error || harness.status !== 0)
    throw Error('Native visibility fixture compilation failed');
  const artifact = harness.stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line))
    .find(
      (value) =>
        value.reason === 'compiler-artifact' &&
        value.profile?.test &&
        value.target?.kind?.includes('lib') &&
        value.executable,
    );
  if (!artifact) throw Error('Native visibility fixture missing');
  const visibility = spawnSync(
    artifact.executable,
    ['--exact', 'windows::lifecycle::tests::hidden_startup_cannot_hide_required_owned_controls'],
    {
      cwd: root,
      windowsHide: true,
      stdio: 'inherit',
      shell: false,
      timeout: 10000,
      env: { ...process.env, PALISADE_UI_HIDDEN_FIXTURE: '1' },
    },
  );
  if (visibility.error || visibility.status !== 0)
    throw Error('Hidden-startup native control visibility failed');
}
main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
