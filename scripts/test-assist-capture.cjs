'use strict';
const { spawnSync } = require('node:child_process');
const path = require('node:path'),
  fs = require('node:fs/promises');
const { flipFuses, getCurrentFuseWire, FuseVersion, FuseV1Options } = require('@electron/fuses');
async function unlocked(operation) {
  // Windows may briefly retain an image/scan lock after the preceding fixture.
  // Retry only sharing violations; every other error and the deadline fail CI.
  for (let attempt = 0; ; attempt++) {
    try { return await operation(); } catch (error) {
      if (!['EBUSY', 'EPERM'].includes(error.code) || attempt >= 40) throw error;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
}
async function main() {
  await require('./build-assist-preload.cjs').buildAssistPreload();
  const root = path.resolve(__dirname, '..'),
    binary = require('electron'),
    fuse = FuseV1Options.GrantFileProtocolExtraPrivileges,
    original = await unlocked(() => getCurrentFuseWire(binary));
  try {
    await unlocked(() => flipFuses(binary, { version: FuseVersion.V1, [fuse]: false }));
    const env = { ...process.env };
    delete env.ELECTRON_RUN_AS_NODE;
    const cases = [
      ['assist-dom-stream-smoke.cjs', [], 'assist-dom-stream-result.json'],
      ['assist-local-video-smoke.cjs', [], 'assist-local-video-result.json'],
      ['assist-local-video-negative.cjs', [], 'assist-local-video-negative-result.json'],
      ['assist-local-video-smoke.cjs', ['--owned-window'], 'assist-owned-window-result.json'],
      ['assist-local-video-smoke.cjs', ['--native-publication'], 'assist-native-capture-result.json'],
    ];
    for (const [file, args, result] of cases) {
      const output = path.join(root, 'out', result);
      await fs.rm(output, { force: true });
      const run = spawnSync(binary, [path.join(__dirname, file), ...args], {
        cwd: root,
        env,
        windowsHide: true,
        stdio: 'inherit',
        timeout: 100000,
        shell: false,
      });
      if (run.error || run.status !== 0) throw Error(file + ' failed');
      const evidence = JSON.parse(await fs.readFile(output, 'utf8'));
      if (evidence.passed !== true || evidence.electron !== '44.7.2')
        throw Error('Missing exact-runtime capture evidence');
    }
    console.log(
      'Isolated capture component fixtures passed with release file-protocol restrictions.',
    );
  } finally {
    await unlocked(() => flipFuses(binary, { version: FuseVersion.V1, [fuse]: original[fuse] === 49 }));
  }
}
main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
