'use strict';
const { isPalisadeUrl } = require('../security-policy.cjs');
const SOURCE_ID = /^(?:screen|window):\d+:\d+$/;
function createCaptureController({ enabled = false, getContents, getWindow, enumerate, choose, onState = () => {},
  revoke = () => {}, timeoutMs = 30_000 }) {
  let pending = null, possiblySharing = false, epoch = 0, disposed = false, lease;
  const available = () => enabled === true && !disposed;
  const busy = () => Boolean(pending || possiblySharing);
  const frameAllowed = request => {
    try {
    const contents = getContents(), window = getWindow();
    return available() && contents && !contents.isDestroyed() && window && !window.isDestroyed() &&
      window.isVisible() && !window.isMinimized() && request?.frame === contents.mainFrame &&
      isPalisadeUrl(contents.getURL()) && isPalisadeUrl(request.frame?.url) &&
      isPalisadeUrl(request.securityOrigin) && request.userGesture === true &&
      request.videoRequested === true && request.audioRequested === false;
    } catch { return false; }
  };
  function stop() {
    epoch++;
    const sharing = possiblySharing;
    possiblySharing = false; clearTimeout(lease);
    if (pending) { pending.abort.abort(); pending.finish(null); }
    // Production revocation must terminate the renderer, not ask page scripts to cooperate.
    try { if (sharing) revoke(); } finally { onState(); }
  }
  async function handle(request, callback) {
    if (!frameAllowed(request) || pending || possiblySharing) { callback(null); return; }
    const generation = epoch, frame = request.frame;
    const abort = new AbortController();
    let settled = false, timer;
    const finish = streams => {
      if (settled) return; settled = true; clearTimeout(timer);
      pending = null; callback(streams); onState();
    };
    pending = { abort, finish };
    timer = setTimeout(stop, timeoutMs); timer.unref?.(); onState();
    try {
      const sources = (await enumerate({ types: ['screen', 'window'], thumbnailSize: { width: 320, height: 180 }, fetchWindowIcons: false }))
        .filter(source => SOURCE_ID.test(source.id)).slice(0, 100);
      if (settled || generation !== epoch || !frameAllowed(request)) { finish(null); return; }
      const selection = await choose(sources, { parent: getWindow(), signal: abort.signal });
      if (settled || !selection || generation !== epoch || frame !== getContents()?.mainFrame || !frameAllowed(request)) { finish(null); return; }
      if (!sources.some(source => source.id === selection)) { finish(null); return; }
      // Re-enumerate at grant time: disappearance or replacement cannot reuse stale metadata.
      const live = await enumerate({ types: ['screen', 'window'], thumbnailSize: { width: 0, height: 0 }, fetchWindowIcons: false });
      const selected = live.find(source => source.id === selection);
      if (settled || generation !== epoch || !frameAllowed(request) || !selected || selected.name !== sources.find(source => source.id === selection).name) { finish(null); return; }
      possiblySharing = true;
      lease = setTimeout(stop, 15 * 60 * 1000); lease.unref?.();
      finish({ video: selected });
    } catch { finish(null); }
  }
  function dispose() { if (disposed) return; stop(); disposed = true; }
  return { available, busy, handle, stop, dispose };
}
module.exports = { createCaptureController, SOURCE_ID };
