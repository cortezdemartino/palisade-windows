'use strict';
// Windows can retain an executable image/scan lock briefly after fixture exit.
// Bound sharing-violation retries; all other failures still fail verification.
async function unlocked(operation) {
  for (let attempt = 0; ; attempt++) {
    try { return await operation(); } catch (error) {
      if (!['EBUSY', 'EPERM'].includes(error.code) || attempt >= 40) throw error;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
}
module.exports = { unlocked };
