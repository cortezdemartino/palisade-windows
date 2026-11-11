'use strict';
// Explicit build-time public trust profile. Produces an unpacked, distinct
// application; never installs, signs, publishes or invokes the candidate.
const fs = require('node:fs/promises'), path = require('node:path');
const { spawnSync } = require('node:child_process');
const { validateQualification } = require('../src/assist/qualification.cjs');
async function build(configFile) {
  if (!configFile) throw Error('Provide the public disposable issuer profile JSON path');
  const root = path.join(__dirname, '..'), profile = path.join(root, 'src/assist/qualification.json');
  const config = validateQualification(JSON.parse(await fs.readFile(configFile, 'utf8')));
  // Refuse an existing concurrent build/profile rather than overwrite it.
  await fs.writeFile(profile, JSON.stringify(config, null, 2) + '\n', { flag: 'wx' });
  try {
    const builder = require.resolve('electron-builder/cli.js');
    const env = { ...process.env, PALISADE_ASSIST_QUALIFICATION_BUILD: '1', CSC_IDENTITY_AUTO_DISCOVERY: 'false' };
    for (const name of ['CSC_LINK', 'WIN_CSC_LINK', 'CSC_KEY_PASSWORD', 'WIN_CSC_KEY_PASSWORD', 'CSC_NAME']) delete env[name];
    const result = spawnSync(process.execPath, [builder, '--win', 'dir', '--x64', '--publish', 'never',
      '--config.appId=org.onyxops.palisade.qualification.' + config.namespace,
      '--config.productName=Palisade Assist Qualification',
      '--config.executableName=Palisade',
      '--config.win.signAndEditExecutable=false',
      '--config.directories.output=out/assist-qualification-' + config.namespace],
    { cwd: root, shell: false, windowsHide: true, stdio: 'inherit',
      env });
    if (result.error || result.status !== 0) throw Error('Qualification packaging failed');
  } finally { await fs.unlink(profile); }
}
module.exports = { build };
if (require.main === module) build(process.argv[2]).catch(error => { console.error(error.message); process.exitCode = 1; });
