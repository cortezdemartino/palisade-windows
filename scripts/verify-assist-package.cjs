'use strict';
const path = require('node:path');
const fs = require('node:fs/promises');
const os = require('node:os');
const { extractFile, listPackage } = require('@electron/asar');
const { verifyGuardian } = require('../src/assist/guardian.cjs');
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
    const main = extractFile(archive, path.normalize('src/main.cjs')).toString('utf8');
    if (/openGuardian|openLiveChannel|assist[\\/](?:guardian|live-channel)\.cjs/.test(main)) throw new Error('Unqualified native helper must not start in production');
  } finally { await fs.rm(temporary, { recursive: true, force: true }); }
}
module.exports = { verifyAssistPackage };
if (require.main === module) verifyAssistPackage(process.argv[2]).catch(error => { console.error(error.message); process.exitCode = 1; });
