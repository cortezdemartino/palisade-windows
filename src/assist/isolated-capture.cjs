'use strict';
const { desktopCapturer } = require('electron');
const { isPalisadeUrl } = require('../security-policy.cjs');
const { selectCaptureSource } = require('./capture.cjs');
const { chooseSource } = require('./source-picker.cjs');
const { createLocalVideoHop } = require('./local-video-hop.cjs');
// Trusted-main component; a negotiated page adapter is a separate integration gate.
function createIsolatedCapture({
  getContents,
  getWindow,
  inspect,
  enabled = false,
  enumerate = (options) => desktopCapturer.getSources(options),
  choose = chooseSource,
  onState = () => {},
}) {
  const consumer = getContents(),
    parent = getWindow();
  let pending = null,
    epoch = 0,
    disposed = false,
    source = null;
  const hop = createLocalVideoHop({
    consumer,
    parent,
    inspect,
    enabled,
    onState: (state, publicOperation) => {
      if (state === 'ended' || state === 'unavailable') {
        epoch++;
        cancelPending();
        source = null;
      }
      onState(state, publicOperation);
    },
  });
  const allowed = () =>
    enabled === true &&
    !disposed &&
    getContents() === consumer &&
    getWindow() === parent &&
    !consumer.isDestroyed() &&
    !parent.isDestroyed() &&
    parent.isVisible() &&
    !parent.isMinimized() &&
    isPalisadeUrl(consumer.getURL());
  function cancelPending() {
    const operation = pending;
    pending = null;
    if (operation) {
      clearTimeout(operation.timer);
      operation.abort.abort();
    }
  }
  function stop() {
    epoch++;
    cancelPending();
    source = null;
    hop.stop();
  }
  const navigation = (_event, _url, inPlace, mainFrame) => {
    if (mainFrame && !inPlace) stop();
  };
  consumer.on('did-start-navigation', navigation);
  consumer.on('render-process-gone', stop);
  consumer.on('destroyed', stop);
  for (const event of ['hide', 'minimize', 'closed']) parent.on(event, stop);
  async function request({ publicOperation = null } = {}) {
    if (!allowed() || pending || hop.busy()) throw Error('Capture selection unavailable');
    const abort = new AbortController(),
      generation = epoch,
      frame = consumer.mainFrame;
    const operation = { abort, timer: null };
    pending = operation;
    const permitted = () =>
      pending === operation &&
      generation === epoch &&
      allowed() &&
      consumer.mainFrame === frame &&
      !abort.signal.aborted;
    operation.timer = setTimeout(() => {
      if (pending === operation && generation === epoch) stop();
    }, 30000);
    operation.timer.unref?.();
    let cancel;
    const cancelled = new Promise((_resolve, reject) => {
      cancel = () => reject(Error('Capture selection cancelled'));
      abort.signal.addEventListener('abort', cancel, { once: true });
    });
    try {
      const work = async () => {
        const selected = await selectCaptureSource({
          enumerate,
          choose,
          parent,
          signal: abort.signal,
          permitted,
        });
        if (!selected || !permitted()) throw Error('Capture selection cancelled');
        const stream = await hop.start({ kind: 'desktop', id: selected.id }, { publicOperation });
        if (!permitted()) {
          // stop() already invalidated this hop. Late completion must not stop a successor.
          throw Error('Capture selection cancelled');
        }
        source = Object.freeze({
          operation: stream.operation,
          id: selected.id,
          kind: selected.id.startsWith('screen:') ? 'screen' : 'window',
          displayId: selected.display_id || null,
        });
        return stream;
      };
      return await Promise.race([work(), cancelled]);
    } finally {
      clearTimeout(operation.timer);
      abort.signal.removeEventListener('abort', cancel);
      if (pending === operation) pending = null;
    }
  }
  function dispose() {
    if (disposed) return;
    disposed = true;
    stop();
    hop.dispose();
    consumer.removeListener('did-start-navigation', navigation);
    consumer.removeListener('render-process-gone', stop);
    consumer.removeListener('destroyed', stop);
    for (const event of ['hide', 'minimize', 'closed']) parent.removeListener(event, stop);
  }
  return {
    request,
    stop,
    dispose,
    busy: () => Boolean(pending) || hop.busy(),
    selected: () => source,
  };
}
module.exports = { createIsolatedCapture };
