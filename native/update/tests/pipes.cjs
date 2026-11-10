'use strict';
// No-authority subprocess fixture. No application, enrollment, staging or installer commands.
const { spawnSync } = require('node:child_process');
const result = spawnSync(process.argv[2], ['--exact', 'windows::parent::tests::pipe_owner_is_bound_to_the_actual_node_creator', '--nocapture'], {
  stdio: 'pipe', encoding: 'utf8', timeout: 10000, windowsHide: true,
  env: { ...process.env, PALISADE_UPDATE_PIPE_CHILD: 'yes', PALISADE_UPDATE_PIPE_PARENT: String(process.pid) },
});
if (result.error || result.status !== 0) {
  process.stderr.write(String(result.error?.message || '') + result.stdout + result.stderr);
  process.exitCode = 1;
}
