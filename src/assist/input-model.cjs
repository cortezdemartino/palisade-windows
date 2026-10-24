'use strict';
// Validates/plans input only. No OS APIs, input injection or keyboard hooks.
const SHORTCUTS = Object.freeze({ copy: ['ControlLeft', 'KeyC'], paste: ['ControlLeft', 'KeyV'],
  selectAll: ['ControlLeft', 'KeyA'], undo: ['ControlLeft', 'KeyZ'], save: ['ControlLeft', 'KeyS'],
  nextTab: ['ControlLeft', 'Tab'], previousTab: ['ControlLeft', 'ShiftLeft', 'Tab'] });
const allowedKey = key => typeof key === 'string' && /^(Key[A-Z]|Digit[0-9]|F(?:[1-9]|1[0-2])|Numpad[0-9]|ControlLeft|ControlRight|ShiftLeft|ShiftRight|AltLeft|AltRight|Enter|Escape|Tab|Space|Backspace|Delete|Insert|Home|End|PageUp|PageDown|ArrowUp|ArrowDown|ArrowLeft|ArrowRight)$/.test(key);
function mapPoint(point, view, source) {
  const values = [point.x, point.y, view.x, view.y, view.width, view.height, source.x, source.y, source.width, source.height];
  if (!values.every(Number.isFinite) || view.width <= 0 || view.height <= 0 || source.width < 1 || source.height < 1 ||
      source.width > 32768 || source.height > 32768 || ![0, 90, 180, 270].includes(source.rotation)) throw new Error('Invalid geometry');
  let x = (point.x - view.x) / view.width, y = (point.y - view.y) / view.height;
  if (x < 0 || x > 1 || y < 0 || y > 1) throw new Error('Outside approved content');
  if (source.rotation === 90) [x, y] = [y, 1 - x];
  if (source.rotation === 180) [x, y] = [1 - x, 1 - y];
  if (source.rotation === 270) [x, y] = [1 - y, x];
  return { x: source.x + Math.round(x * (source.width - 1)), y: source.y + Math.round(y * (source.height - 1)) };
}
class InputLedger {
  constructor() { this.keys = new Set(); this.buttons = new Set(); }
  key(key, down) {
    if (!allowedKey(key) || typeof down !== 'boolean') throw new Error('Unsupported key');
    // Alt+Tab, OS/meta keys and secure combinations remain unimplemented pending review.
    if (down && key === 'Tab' && (this.keys.has('AltLeft') || this.keys.has('AltRight'))) throw new Error('Shortcut not approved');
    if (down && key.startsWith('Alt') && this.keys.has('Tab')) throw new Error('Shortcut not approved');
    const next = new Set(this.keys); if (down) next.add(key);
    if (next.has('Delete') && ['ControlLeft', 'ControlRight'].some(v => next.has(v)) && ['AltLeft', 'AltRight'].some(v => next.has(v))) throw new Error('Protected shortcut');
    if (down && this.keys.size >= 16 && !this.keys.has(key)) throw new Error('Held key limit');
    down ? this.keys.add(key) : this.keys.delete(key);
    return { key, down };
  }
  button(button, down) {
    if (!['left', 'middle', 'right'].includes(button) || typeof down !== 'boolean') throw new Error('Unsupported button');
    down ? this.buttons.add(button) : this.buttons.delete(button); return { button, down };
  }
  releaseAll() {
    const releases = [...this.keys].reverse().map(key => ({ key, down: false }))
      .concat([...this.buttons].map(button => ({ button, down: false })));
    this.keys.clear(); this.buttons.clear(); return releases;
  }
}
module.exports = { mapPoint, InputLedger, SHORTCUTS, allowedKey };
