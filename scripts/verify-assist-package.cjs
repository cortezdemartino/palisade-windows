'use strict';
const path = require('node:path');
const fs = require('node:fs/promises');
const os = require('node:os');
const { spawnSync } = require('node:child_process');
const { extractFile, listPackage } = require('@electron/asar');
const { verifyGuardian } = require('../src/assist/guardian.cjs');
const CLEANUP_TEST = 'actual helper build contains cleanup panic, keeps failure sticky and joins later work';
function verifiedCleanupReport(report) {
  if (typeof report !== 'string') return false;
  const lines = report.split(/\r?\n/);
  return lines.includes('# Subtest: ' + CLEANUP_TEST) &&
    lines.some(line => /^ok [1-9][0-9]* - (.*)$/.exec(line)?.[1] === CLEANUP_TEST) &&
    ['# pass 1', '# fail 0', '# cancelled 0'].every(line => lines.includes(line));
}
async function verifyAssistPackage(resources = path.resolve('out/win-unpacked/resources')) {
  const archive = path.join(resources, 'app.asar');
  const entries = listPackage(archive).map(name => name.replaceAll('\\', '/'));
  if (entries.some(name => /\/(?:native|tests|scripts|target)\//.test(name) || /\.dpapi$|private.*\.pem$/i.test(name))) {
    throw new Error('Unexpected native sources, tests or secret material in application archive');
  }
  const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'palisade-assist-package-'));
  try {
    const manifest = path.join(temporary, 'manifest.json');
    await fs.writeFile(manifest, extractFile(archive, path.normalize('src/assist/guardian-integrity.json')));
    await verifyGuardian(path.join(resources, 'assist/Palisade-Assist.exe'), manifest);
    // Reuse the fixed no-authority fixture against the bytes actually packaged,
    // so release/profile overrides cannot make panic recovery debug-only.
    const probe = spawnSync(process.execPath, ['--test', '--test-reporter=tap', '--test-name-pattern=^' + CLEANUP_TEST + '$',
      path.join(__dirname, '../tests/assist-live-process.native.cjs')], {
      windowsHide: true, shell: false, timeout: 12000, maxBuffer: 1024 * 1024,
      stdio: 'pipe', encoding: 'utf8', env: { ...process.env, PALISADE_ASSIST_TEST_BINARY: path.resolve(resources, 'assist/Palisade-Assist.exe') },
    });
    if (probe.error || probe.status !== 0 || !verifiedCleanupReport(probe.stdout)) throw Error('Packaged native cleanup probe failed');
    const main = extractFile(archive, path.normalize('src/main.cjs')).toString('utf8');
    if (/openGuardian|openLiveChannel|assist[\\/](?:guardian|live-channel)\.cjs/.test(main)) throw new Error('Unqualified native helper must not start in production');
  } finally { await fs.rm(temporary, { recursive: true, force: true }); }
}
module.exports = { verifyAssistPackage, verifiedCleanupReport };
if (require.main === module) verifyAssistPackage(process.argv[2]).catch(error => { console.error(error.message); process.exitCode = 1; });
