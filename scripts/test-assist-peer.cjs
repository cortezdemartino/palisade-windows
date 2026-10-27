'use strict';
const { spawnSync } = require('node:child_process');
const path = require('node:path'), fs = require('node:fs/promises');
const { flipFuses, getCurrentFuseWire, FuseVersion, FuseV1Options } = require('@electron/fuses');
async function main() {
  const binary = require('electron'), fuse = FuseV1Options.GrantFileProtocolExtraPrivileges;
  const original = await getCurrentFuseWire(binary), root = path.resolve(__dirname, '..');
  const output = path.join(root, 'out/assist-remote-input-result.json');
  await fs.mkdir(path.dirname(output), { recursive: true });
  await fs.rm(output, { force: true });
  try {
    await flipFuses(binary, { version: FuseVersion.V1, [fuse]: false });
    const env = { ...process.env }; delete env.ELECTRON_RUN_AS_NODE;
    const run = spawnSync(binary, [path.join(__dirname, 'assist-remote-input-smoke.cjs')], {
      cwd: root, env, windowsHide: true, stdio: 'inherit', timeout: 120000, shell: false });
    if (run.error || run.status !== 0) throw Error(`Owned remote peer fixtures failed (exit ${run.status}, signal ${run.signal ?? 'none'})`);
    const evidence = JSON.parse(await fs.readFile(output, 'utf8'));
    if (evidence.passed !== true || evidence.electron !== '44.7.2' || evidence.inputInjected !== false || evidence.cases.length !== 14)
      throw Error('Missing exact-runtime peer evidence');
  } finally { await flipFuses(binary, { version: FuseVersion.V1, [fuse]: original[fuse] === 49 }); }
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
