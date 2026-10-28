'use strict';
// Trusted-main ownership. A holder must be registered before it can admit native
// authority. Its wait() resolves only after its real owned cleanup/child exit.
const incomplete = () => Object.assign(Error('Assist cleanup incomplete'), { code: 'assist-cleanup-incomplete' });
function createAssistLifecycle() {
  const holders = new Set(), abort = new AbortController();
  let state = 'open', pending = null;
  function revoke(entry, reason) {
    entry.abort.abort();
    if (entry.revoked || entry.gone) return null;
    try {
      entry.revoked = true;
      const result = entry.revoke(reason);
      if (result !== undefined) {
        // Do not mistake an async revoke for synchronous invalidation.
        if (typeof result?.catch === 'function') void result.catch(() => {});
        throw incomplete();
      }
      return null;
    } catch { entry.revoked = false; return incomplete(); }
  }
  function join(entry) {
    if (entry.gone) return Promise.resolve();
    if (entry.pending) return entry.pending;
    entry.pending = Promise.resolve().then(async () => {
      const wait = entry.wait();
      if (!wait || typeof wait.then !== 'function' || await wait !== undefined) throw incomplete();
      entry.gone = true;
      holders.delete(entry);
    }).finally(() => { entry.pending = null; });
    return entry.pending;
  }
  function hold({ revoke: requestRevoke, wait } = {}) {
    if (state !== 'open' || abort.signal.aborted) throw incomplete();
    if (holders.size >= 64 || typeof requestRevoke !== 'function' || typeof wait !== 'function') throw incomplete();
    const entry = { revoke: requestRevoke, wait, abort: new AbortController(), revoked: false, gone: false, pending: null };
    holders.add(entry);
    return Object.freeze({
      signal: entry.abort.signal,
      revoke: reason => { if (revoke(entry, reason)) throw incomplete(); },
      finish: async (reason = 'ended') => {
        const failure = revoke(entry, reason);
        await join(entry);
        if (failure) throw failure;
      },
    });
  }
  function quiesce(reason = 'quit') {
    if (pending) return pending;
    if (state === 'closed') return Promise.resolve();
    // Publish the barrier before callbacks; abort/revoke can reenter quiesce.
    let resolve, reject;
    const result = new Promise((done, fail) => { resolve = done; reject = fail; });
    pending = result;
    state = 'closing';
    abort.abort();
    const entries = [...holders];
    // Revoke EVERY holder synchronously before awaiting any cleanup.
    const failed = entries.map(entry => revoke(entry, reason)).some(Boolean);
    void Promise.allSettled(entries.map(join)).then(results => {
      pending = null;
      if (failed || results.some(value => value.status !== 'fulfilled') || holders.size) {
        state = 'blocked'; reject(incomplete());
      } else { state = 'closed'; resolve(); }
    });
    return result;
  }
  return Object.freeze({ hold, quiesce, signal: abort.signal,
    accepting: () => state === 'open', status: () => ({ state, holders: holders.size }) });
}
// Electron's event is synchronous; the second quit is allowed only after the
// shared preparation barrier and its final profile-storage work complete.
function installQuitBarrier(app, { prepare, onApproved = () => {}, onBlocked = () => {}, onWaiting = () => {}, noticeAfterMs = 2000 } = {}) {
  if (typeof prepare !== 'function') throw Error('Exit preparation required');
  if (!Number.isInteger(noticeAfterMs) || noticeAfterMs < 1 || noticeAfterMs > 60000) throw Error('Invalid exit notice delay');
  let approved = false, pending = null;
  app.on('before-quit', event => {
    if (approved) return;
    event.preventDefault();
    if (pending) return;
    // Publish before synchronous revocation: callbacks may reenter app.quit().
    let settle;
    const attempt = new Promise(resolve => { settle = resolve; });
    pending = attempt;
    const notice = setTimeout(() => {
      if (pending !== attempt || approved) return;
      try { const result = onWaiting(); if (result?.catch) void result.catch(() => {}); } catch {}
    }, noticeAfterMs);
    notice.unref?.();
    let preparation;
    try {
      preparation = prepare('quit');
      if (!preparation || typeof preparation.then !== 'function') throw incomplete();
    } catch { preparation = Promise.reject(incomplete()); }
    void Promise.resolve(preparation).then(async value => {
      if (value !== undefined) throw incomplete();
      await onApproved(); approved = true; app.quit();
    }).catch(() => {
      approved = false;
      try { const result = onBlocked(); if (result?.catch) void result.catch(() => {}); } catch {}
    }).finally(() => { clearTimeout(notice); if (pending === attempt) pending = null; settle(); });
  });
}
module.exports = { createAssistLifecycle, installQuitBarrier };
