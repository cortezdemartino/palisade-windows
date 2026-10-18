'use strict';
const { ipcRenderer } = require('electron');
// Isolated, one-way observation only: expose no bridge and read no account/session storage.
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
