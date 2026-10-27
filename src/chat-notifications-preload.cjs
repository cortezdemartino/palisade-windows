'use strict';
const { ipcRenderer, contextBridge } = require('electron');
// Isolated unread observation reads no account/session storage. The v1 description API is below.
let previous = null, timer, firstSeen = 0;
function report() {
  const rail = document.querySelector('nav[aria-label="Workspaces"]');
  const live = rail?.querySelector('button[aria-label="Create a workspace"]');
  if (!live) {
    if (previous !== null) ipcRenderer.send('palisade:unread-state', { count: 0, baseline: true });
    previous = null; firstSeen = 0; return;
  }
  if (previous === null) firstSeen = Date.now();
  let count = 0;
  for (const button of rail.querySelectorAll('button.rail__item')) {
    if (button.classList.contains('is-invite') || !button.querySelector('.rail__badge')) continue;
    const match = /(?:^|, )(\d+) unread$/.exec(button.getAttribute('aria-label') || '');
    if (match) count = Math.min(9999, count + Math.min(9999, Number(match[1])));
  }
  if (count !== previous) ipcRenderer.send('palisade:unread-state', { count, baseline: previous === null || Date.now() - firstSeen < 5000 });
  previous = count;
}

// Separately negotiated native channel. Production passes no enabling argument
// until its compiled coordinator, native authority and acceptance gates are met.
if (typeof process !== 'undefined' && process.isMainFrame && location.origin === 'https://palisade.onyxops.org' && Array.isArray(process.argv) && process.argv.includes('--palisade-assist-native')) {
  const documentId = crypto.randomUUID(), listeners = new Set(), calls = new Set();
  let alive = true, lastSequence = 0;
  const connect = 'palisade:assist-native-connect', request = 'palisade:assist-native-request', expired = 'palisade:assist-native-expired', probe = 'palisade:assist-native-probe', witness = 'palisade:assist-native-witness', eventChannel = 'palisade:assist-native-event';
  const nativeProtocol = 'palisade.assist.native';
  const answerProbe = (_event, value) => {
    if (alive && value && typeof value.challenge === 'string' && /^[a-f0-9]{48}$/.test(value.challenge)) ipcRenderer.send(witness, { document: documentId, challenge: value.challenge });
  };
  ipcRenderer.on(probe, answerProbe);
  const connected = ipcRenderer.invoke(connect, { document: documentId }).catch(() => false);
  const receive = (_event, envelope) => {
    if (!envelope || Object.keys(envelope).sort().join(',') !== 'document,text' || envelope.document !== documentId) return;
    const text = envelope.text;
    if (!alive || typeof text !== 'string' || text.length > 32768) return;
    let value; try { value = JSON.parse(text); } catch { return; }
    if (value?.p !== nativeProtocol || value.v !== 1 || !['peer.candidate', 'capture.status', 'lifecycle'].includes(value.event) || !Number.isSafeInteger(value.seq) || value.seq <= lastSequence) return;
    lastSequence = value.seq;
    for (const listener of listeners) { try { listener(text); } catch {} }
  };
  ipcRenderer.on(eventChannel, receive);
  contextBridge.exposeInMainWorld('palisadeAssistNative', {
    request: async text => {
      if (!alive || typeof text !== 'string' || !text.length || text.length > 32768) throw Error('Invalid native message');
      let value; try { value = JSON.parse(text); } catch { throw Error('Invalid native message'); }
      const safety = ['stop', 'capture.stop', 'capture.cancel', 'peer.close'].includes(value?.op);
      if (!/^[A-Za-z0-9_-]{8,64}$/.test(value?.id || '') ||
          (!safety && [...calls].filter(call => !call.safety).length >= 8) ||
          (safety && value.op !== 'stop' && calls.size >= 12) ||
          (value.op === 'stop' && [...calls].some(call => call.op === 'stop'))) throw Error('Native Assist busy');
      const call = { done: false, safety, op: value.op }; calls.add(call);
      let timer;
      try {
        return await Promise.race([
          (async () => {
            if (await connected !== true || !alive || call.done) throw Error('Native Assist unavailable');
            const result = await ipcRenderer.invoke(request, { document: documentId, text });
            if (!alive || call.done || typeof result !== 'string' || result.length > 32768) throw Error('Native Assist unavailable');
            return result;
          })(),
          new Promise((_resolve, reject) => { call.reject = reject; timer = call.timer = setTimeout(() => {
            call.done = true;
            ipcRenderer.send(expired, { document: documentId, id: value.id });
            reject(Error('Native Assist timeout'));
          }, 120000); }),
        ]);
      } finally { call.done = true; clearTimeout(timer); calls.delete(call); }
    },
    subscribe: listener => {
      if (!alive || typeof listener !== 'function' || listeners.size >= 8) return () => {};
      listeners.add(listener); return () => listeners.delete(listener);
    },
  });
  window.addEventListener('unload', () => {
    alive = false; listeners.clear();
    for (const call of calls) { call.done = true; clearTimeout(call.timer); call.reject?.(Error('Native Assist stopped')); }
    ipcRenderer.removeListener(probe, answerProbe); ipcRenderer.removeListener(eventChannel, receive);
  }, { once: true });
}
window.addEventListener('DOMContentLoaded', () => {
  const observer = new MutationObserver(() => { clearTimeout(timer); timer = setTimeout(report, 150); });
  observer.observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ['aria-label', 'class'] });
  report();
  window.addEventListener('unload', () => { observer.disconnect(); clearTimeout(timer); }, { once: true });
}, { once: true });

// Canonical v1 bridge: descriptions only; no session, file, clipboard or input operations.
if (typeof process !== 'undefined' && process.isMainFrame && location.origin === 'https://palisade.onyxops.org') {
  const protocol = 'palisade.assist.bridge';
  const listeners = new Set();
  const valid = value => value && typeof value === "object" && !Array.isArray(value) &&
    [Object.prototype, null].includes(Object.getPrototypeOf(value)) &&
    Object.keys(value).sort().join(",") === "protocol,versions" && value.protocol === protocol &&
    Array.isArray(value.versions) && value.versions.length > 0 && value.versions.length <= 8 &&
    value.versions.every(v => Number.isInteger(v) && v > 0 && v <= 65535);
  const accept = value => value && typeof value === "object" &&
    Object.keys(value).sort().join(",") === "capture,desktopVersion,emergencyStop,localRevocation,nativeEndpoint,protocol,version" &&
    value.protocol === protocol && value.version === 1 && /^\d{1,4}\.\d{1,4}\.\d{1,6}$/.test(value.desktopVersion) &&
    ["desktop-picker", "unavailable"].includes(value.capture) && value.nativeEndpoint === "absent" &&
    value.localRevocation === false && value.emergencyStop === false;
  const changed = (_event, value) => { if (accept(value)) for (const listener of listeners) { try { listener(value); } catch {} } };
  ipcRenderer.on("palisade:assist-capabilities-changed", changed);
  contextBridge.exposeInMainWorld("palisadeDesktopAssist", {
    protocol,
    getCapabilities: async request => {
      if (!valid(request)) throw new Error("Invalid Assist request");
      let timer;
      try {
        const value = await Promise.race([ipcRenderer.invoke("palisade:assist-capabilities", { versions: [...request.versions] }),
          new Promise((_resolve, reject) => { timer = setTimeout(() => reject(new Error("Assist timeout")), 2500); })]);
        if (!accept(value)) throw new Error("Invalid Assist report");
        return value;
      } finally { clearTimeout(timer); }
    },
    onCapabilitiesChanged: listener => {
      if (typeof listener !== "function" || listeners.size >= 8) return () => {};
      listeners.add(listener); return () => listeners.delete(listener);
    },
  });
  window.addEventListener("unload", () => { listeners.clear(); ipcRenderer.removeListener("palisade:assist-capabilities-changed", changed); }, { once: true });
}
