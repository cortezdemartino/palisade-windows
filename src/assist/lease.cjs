'use strict';
// Offline endpoint admission component. No renderer route or authority implementation uses it yet.
const CAPABILITIES = Object.freeze(['view_screen', 'send_pointer', 'annotate_screen', 'clear_annotations',
  'control_mouse', 'control_keyboard', 'send_shortcut', 'clipboard_send_text', 'clipboard_receive_text',
  'clipboard_send_image', 'clipboard_receive_image', 'file_send', 'file_receive']);
const BINDING = ['sessionId', 'hostAccount', 'hostDevice', 'guestAccount', 'guestDevice', 'endpointId'];
class AssistLease {
  constructor({ verifyGrant, now = Date.now, onRevoke = () => {} }) {
    if (typeof verifyGrant !== 'function') throw new Error('Independent verifier required');
    this.verify = verifyGrant; this.now = now; this.onRevoke = onRevoke;
    this.state = 'inactive'; this.consent = new Set(); this.sequences = new Map(); this.rates = new Map();
    this.lastTime = now(); this.grant = null;
  }
  bind(envelope, expected) {
    if (this.state !== 'inactive') throw new Error('Fresh endpoint required');
    let grant;
    try { grant = this.verify(envelope); } catch { this.revoke(); throw new Error("Grant verification failed"); }
    const time = this.now();
    if (!grant || BINDING.some(key => typeof grant[key] !== 'string' || grant[key].length > 320 ||
      !grant[key].length || grant[key] !== expected[key]) ||
      !Number.isSafeInteger(grant.issuedAt) || !Number.isSafeInteger(grant.expiresAt) ||
      grant.issuedAt > time || grant.expiresAt <= time || grant.expiresAt - grant.issuedAt > 900_000 ||
      !Number.isInteger(grant.generation) || grant.generation < 1 ||
      typeof grant.grantId !== 'string' || !/^[A-Za-z0-9_-]{16,128}$/.test(grant.grantId) ||
      typeof grant.nonce !== 'string' || !/^[A-Za-z0-9_-]{16,128}$/.test(grant.nonce) ||
      !Array.isArray(grant.capabilities) || !grant.capabilities.length || grant.capabilities.length > CAPABILITIES.length ||
      new Set(grant.capabilities).size !== grant.capabilities.length || grant.capabilities.some(cap => !CAPABILITIES.includes(cap))) {
      this.revoke(); throw new Error('Grant refused');
    }
    this.grant = Object.freeze({ ...grant, capabilities: Object.freeze([...grant.capabilities]) });
    this.timer = setTimeout(() => this.revoke(), grant.expiresAt - time); this.timer.unref?.();
    this.lastTime = time; this.state = 'consent-pending';
  }
  consentTo(capabilities) {
    if (this.state !== 'consent-pending' || !Array.isArray(capabilities) ||
        capabilities.some(cap => !this.grant.capabilities.includes(cap))) throw new Error('Consent refused');
    this.consent = new Set(capabilities); this.state = 'session-bound';
  }
  check(capability, message) {
    const time = this.now();
    if (time < this.lastTime || (this.grant && time >= this.grant.expiresAt)) this.revoke();
    this.lastTime = time;
    const grant = this.grant;
    if (this.state !== 'session-bound' || !grant || !this.consent.has(capability) ||
      !message || message.sessionId !== grant.sessionId || message.grantId !== grant.grantId ||
      message.generation !== grant.generation || message.nonce !== grant.nonce ||
      !Number.isSafeInteger(message.sequence) || message.sequence < 0 || message.sequence > 0xffffffff ||
      message.sequence <= (this.sequences.get(capability) ?? -1)) throw new Error('Operation refused');
    const rate = this.rates.get(capability);
    const bucket = rate && time - rate.since < 1000 ? rate : { since: time, count: 0 };
    if (++bucket.count > 120) { this.revoke(); throw new Error('Rate limit'); }
    this.rates.set(capability, bucket); this.sequences.set(capability, message.sequence);
    return grant;
  }
  revoke() {
    if (this.state === 'terminated') return;
    clearTimeout(this.timer);
    this.state = 'terminated'; this.grant = null; this.consent.clear(); this.sequences.clear(); this.rates.clear();
    this.onRevoke();
  }
}
module.exports = { AssistLease, CAPABILITIES };
