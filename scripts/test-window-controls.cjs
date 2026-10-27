'use strict';
const { spawn } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs/promises');
const { flipFuses, getCurrentFuseWire, FuseVersion, FuseV1Options } = require('@electron/fuses');
async function main() {
  const executable = require('electron');
  const fuse = FuseV1Options.GrantFileProtocolExtraPrivileges;
  const original = await getCurrentFuseWire(executable);
  try {
    // Exercise the same restricted file loading used by release builds.
    await flipFuses(executable, { version: FuseVersion.V1, [fuse]: false });
    const env = { ...process.env };
    delete env.ELECTRON_RUN_AS_NODE;
    for (const [script, result] of [
      ['window-controls-smoke.cjs', 'tabler-controls-result.json'],
      ['chat-notifications-smoke.cjs', 'chat-notifications-result.json'],
      ['session-policy-smoke.cjs', 'session-policy-result.json'],
      ['assist-smoke.cjs', 'assist-smoke-result.json'],
      ['capture-admission-smoke.cjs', 'capture-admission-result.json'],
    ]) {
      const output = path.join(__dirname, '..', 'out', result);
      await fs.rm(output, { force: true });
      const code = await new Promise((resolve, reject) => {
        const child = spawn(executable, [path.join(__dirname, script)], {
          cwd: path.join(__dirname, '..'), env, stdio: 'inherit', windowsHide: true,
        });
        child.once('error', reject); child.once('exit', resolve);
      });
      if (code !== 0) throw new Error(script + ' failed.');
      const evidence = JSON.parse(await fs.readFile(output, 'utf8'));
      const runtime = evidence.electron || evidence.runtime;
      if (evidence.passed !== true || (runtime && runtime !== require('../package.json').devDependencies.electron)) {
        throw new Error(script + ' did not produce fresh passing evidence for the pinned runtime.');
      }
    }
    console.log('Window controls, unread observer and permission boundaries pass with release file-protocol restrictions.');
  } finally {
    await flipFuses(executable, { version: FuseVersion.V1, [fuse]: original[fuse] === 49 });
  }
}
main().catch((error) => { console.error(error); process.exitCode = 1; });
