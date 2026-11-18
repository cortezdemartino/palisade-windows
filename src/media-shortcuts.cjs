'use strict';
const { accelerator } = require('./media-settings.cjs');
function createMediaShortcuts({ globalShortcut, dispatch, available = () => false, focused = () => false, onStatus = () => {} }) {
  let settings = null, stopped = false, generation = 0;
  const registered = new Set(), held = new Set(), failures = new Set();
  const unregister = () => { for (const key of registered) globalShortcut.unregister(key); registered.clear(); held.clear(); };
  function fire(action) { if (!stopped && available(action)) dispatch(action); }
  function configure(next, fromFocus = false) {
    const epoch = ++generation;
    unregister(); if (!fromFocus) failures.clear(); settings = next;
    if (stopped || !next.backgroundShortcuts || focused()) { onStatus([...failures]); return; }
    failures.clear();
    for (const [action, binding] of Object.entries(next.keybinds)) {
      if (action === 'push-to-talk' || !binding.enabled || !binding.accelerator || !available(action)) continue;
      const key = accelerator(binding.accelerator);
      try {
        if (globalShortcut.register(key, () => { if (epoch === generation && !focused()) fire(action); }) !== true) failures.add(action);
        else registered.add(key);
      } catch { failures.add(action); }
    }
    onStatus([...failures]);
  }
  function input(event, value) {
    if (stopped || !settings || !focused()) return;
    // No plain typing shortcut and no OS keyboard hook. Only matched action
    // codes are retained until release; text/ordinary held keys are not stored.
    if (value.type === 'keyUp') { held.delete(value.code); return; }
    if (value.isAutoRepeat || value.isComposing || value.meta) return;
    if (value.type !== 'keyDown' || held.has(value.code)) return;
    const key = value.key?.length === 1 ? value.key.toUpperCase() : ({ ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right', ' ': 'Space' }[value.key] || value.key);
    let match;
    try { match = accelerator([value.control && 'Control', value.alt && 'Alt', value.shift && 'Shift', key].filter(Boolean).join('+')); } catch { return; }
    for (const [action, binding] of Object.entries(settings.keybinds)) {
      if (action !== 'push-to-talk' && binding.enabled && binding.accelerator === match && available(action)) {
        held.add(value.code);
        event.preventDefault(); fire(action); return;
      }
    }
  }
  const refreshFocus = () => { held.clear(); if (settings && !stopped) configure(settings, true); };
  return Object.freeze({ configure, input, focus: refreshFocus, blur: refreshFocus, failures: () => [...failures],
    stop() { stopped = true; generation++; unregister(); }, });
}
module.exports = { createMediaShortcuts };
