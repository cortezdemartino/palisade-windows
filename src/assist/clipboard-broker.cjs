'use strict';
const { createHash } = require('node:crypto');
const MODES = new Set(['off', 'send', 'receive', 'both']);
function textValue(value) {
  if (typeof value !== 'string' || value.includes('\0') || /[\uD800-\uDFFF]/u.test(value) || Buffer.byteLength(value, 'utf8') > 65_536) {
    throw new Error('Invalid clipboard text');
  }
  return value;
}
const digest = text => createHash('sha256').update(text, 'utf8').digest('hex');
class ClipboardBroker {
  // Owned only by trusted main/local UI. No path, token, approval boolean or clipboard object reaches the hosted page.
  constructor({ clipboard, authorize, send, signal, role = 'publisher', sendSignal, receiveSignal, syncAgreement = () => false, onState = () => {}, schedule = setInterval, cancel = clearInterval }) {
    if (!clipboard || typeof authorize !== 'function' || typeof send !== 'function' || !signal) throw new Error('Clipboard authority required');
    if (!['publisher', 'helper'].includes(role)) throw new Error('Invalid clipboard role');
    this.sendCap = role === 'publisher' ? 'clipboard_to_guest' : 'clipboard_to_host';
    this.receiveCap = role === 'publisher' ? 'clipboard_to_host' : 'clipboard_to_guest';
    this.sendSignal = sendSignal; this.receiveSignal = receiveSignal; this.epoch = 0;
    Object.assign(this, { clipboard, authorize, send, signal, syncAgreement, onState, schedule, cancel });
    this.mode = 'off'; this.pending = null; this.lastSent = null; this.lastReceived = null; this.busy = false; this.closed = false;
    this.abort = () => this.close(); signal.addEventListener('abort', this.abort, { once: true });
    if (signal.aborted) this.close();
    this.sendRevoked = () => { this.pause(); this.lastSent = null; };
    this.receiveRevoked = () => { this.pause(); this.pending = null; this.lastReceived = null; this.publish(); };
    sendSignal?.addEventListener('abort', this.sendRevoked, { once: true });
    receiveSignal?.addEventListener('abort', this.receiveRevoked, { once: true });
  }
  live() { if (this.closed || this.signal.aborted) throw new Error('Clipboard session ended'); }
  async allowed(capability, check = () => {}) {
    this.live(); check();
    const directional = capability === this.sendCap ? this.sendSignal : this.receiveSignal;
    if (directional?.aborted || await this.authorize(capability) !== true) throw new Error('Clipboard permission denied');
    this.live(); check(); if (directional?.aborted) throw new Error('Clipboard permission revoked');
  }
  publish() { try { this.onState({ mode: this.mode, incoming: Boolean(this.pending), available: !this.closed }); } catch {} }
  // Only a trusted local menu/button calls these methods; remote packets cannot set the mode.
  async setMode(mode) {
    this.live(); if (!MODES.has(mode)) throw new Error('Invalid clipboard mode');
    this.pause(); const epoch = this.epoch;
    const check = () => { if (this.epoch !== epoch || this.syncAgreement() !== true) throw new Error('Clipboard sync ended'); };
    if (mode !== 'off' && this.syncAgreement() !== true) throw new Error('Clipboard sync is not negotiated');
    if (mode === 'send' || mode === 'both') await this.allowed(this.sendCap, check); this.live();
    if (mode === 'receive' || mode === 'both') await this.allowed(this.receiveCap, check); this.live();
    this.mode = mode; this.cancel(this.timer); this.timer = null;
    if (mode === 'send' || mode === 'both') this.timer = this.schedule(() => { void this.syncOnce().catch(() => this.pause()); }, 500);
    this.publish();
  }
  pause() { this.epoch++; this.cancel(this.timer); this.timer = null; this.mode = 'off'; this.publish(); }
  async userSend() {
    await this.allowed(this.sendCap); this.live();
    const text = textValue(this.clipboard.readText());
    await this.allowed(this.sendCap); this.live();
    await this.send(text); this.live(); this.lastSent = digest(text); this.publish();
  }
  async syncOnce() {
    if (this.busy || !['send', 'both'].includes(this.mode)) return;
    this.busy = true;
    const epoch = this.epoch;
    const check = () => { if (this.epoch !== epoch || !['send', 'both'].includes(this.mode) || this.syncAgreement() !== true) throw new Error('Clipboard sync ended'); };
    try {
      await this.allowed(this.sendCap, check); this.live();
      const text = textValue(this.clipboard.readText()), hash = digest(text);
      if (hash === this.lastSent || hash === this.lastReceived) return;
      await this.allowed(this.sendCap, check); this.live();
      await this.send(text); this.live(); this.lastSent = hash;
    } finally { this.busy = false; }
  }
  async receive(value) {
    await this.allowed(this.receiveCap); this.live(); const text = textValue(value), hash = digest(text);
    if (hash === this.lastReceived) return;
    // Manual receive is staged in memory only. One item, no automatic history.
    this.pending = { text, hash };
    if (this.mode === 'receive' || this.mode === 'both') await this.userCopyIncoming(this.epoch);
    this.publish();
  }
  async userCopyIncoming(automaticEpoch) {
    const check = () => { if (automaticEpoch !== undefined && (this.epoch !== automaticEpoch || !['receive', 'both'].includes(this.mode) || this.syncAgreement() !== true)) throw new Error('Clipboard sync ended'); };
    await this.allowed(this.receiveCap, check); this.live(); const item = this.pending;
    if (!item) return;
    await this.allowed(this.receiveCap, check); this.live();
    this.clipboard.writeText(item.text); this.lastReceived = item.hash;
    if (this.pending === item) this.pending = null;
    this.publish();
  }
  discardIncoming() { this.pending = null; this.publish(); }
  close() {
    if (this.closed) return;
    this.closed = true; this.epoch++; this.cancel(this.timer); this.timer = null; this.mode = 'off'; this.pending = null;
    this.sendSignal?.removeEventListener('abort', this.sendRevoked); this.receiveSignal?.removeEventListener('abort', this.receiveRevoked);
    this.lastSent = null; this.lastReceived = null; this.signal.removeEventListener('abort', this.abort); this.publish();
  }
}
module.exports = { ClipboardBroker, textValue };
