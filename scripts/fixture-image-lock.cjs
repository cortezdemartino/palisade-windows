'use strict';
// Windows can retain an executable image/scan lock briefly after fixture exit.
// Bound sharing-violation retries; all other failures still fail verification.
const path = require('node:path');
async function unlocked(operation, executable = null) {
  for (let attempt = 0; ; attempt++) {
    try { return await operation(); } catch (error) {
      // Some Windows filter/image locks surface as libuv UNKNOWN on open.
      // Retry only this exact fixture image, never arbitrary I/O or test errors.
      const fixtureOpen = process.platform === 'win32' && executable &&
        error.code === 'UNKNOWN' && error.errno === -4094 && error.syscall === 'open' &&
        typeof error.path === 'string' && path.resolve(error.path).toLowerCase() === path.resolve(executable).toLowerCase();
      if ((!['EBUSY', 'EPERM'].includes(error.code) && !fixtureOpen) || attempt >= 40) throw error;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
}
module.exports = { unlocked };
