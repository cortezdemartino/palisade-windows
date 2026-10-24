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
