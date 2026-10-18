'use strict';
const { spawn } = require('node:child_process');
const path = require('node:path');
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
    const code = await new Promise((resolve, reject) => {
      const child = spawn(executable, [path.join(__dirname, 'window-controls-smoke.cjs')], {
        cwd: path.join(__dirname, '..'), env, stdio: 'inherit', windowsHide: true,
      });
      child.once('error', reject);
      child.once('exit', resolve);
    });
    if (code !== 0) throw new Error('Window controls smoke test failed.');
    const chatCode = await new Promise((resolve, reject) => {
      const child = spawn(executable, [path.join(__dirname, 'chat-notifications-smoke.cjs')], {
        cwd: path.join(__dirname, '..'), env, stdio: 'inherit', windowsHide: true,
      });
      child.once('error', reject); child.once('exit', resolve);
    });
    if (chatCode !== 0) throw new Error('Chat notification observer smoke test failed.');
    console.log('Visible icons and window actions pass with release file-protocol restrictions.');
  } finally {
    await flipFuses(executable, { version: FuseVersion.V1, [fuse]: original[fuse] === 49 });
  }
}
main().catch((error) => { console.error(error); process.exitCode = 1; });
