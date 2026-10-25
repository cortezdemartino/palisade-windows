'use strict';
// Offline endpoint admission component. No renderer route or authority implementation uses it yet.
const CAPABILITIES = Object.freeze(['view_screen', 'send_pointer', 'annotate_screen', 'clear_annotations',
  'control_mouse', 'control_keyboard', 'send_shortcut', 'clipboard_send_text', 'clipboard_receive_text',
  'clipboard_send_image', 'clipboard_receive_image', 'file_send', 'file_receive']);
const BINDING = ['sessionId', 'hostAccount', 'hostDevice', 'guestAccount', 'guestDevice', 'endpointId'];
const validTime = value => Number.isSafeInteger(value) && value >= 0;
class AssistLease {
  constructor({ verifyGrant, now = Date.now, onRevoke = () => {} }) {
    if (typeof verifyGrant !== 'function') throw new Error('Independent verifier required');
    this.verify = verifyGrant; this.now = now; this.onRevoke = onRevoke;
    this.state = 'inactive'; this.consent = new Set(); this.sequences = new Map(); this.rates = new Map();
    this.lastTime = now(); this.grant = null;
    if (!validTime(this.lastTime)) throw new Error('Invalid clock');
    this.cancellation = new AbortController(); this.capabilityCancellations = new Map();
  }
  get signal() { return this.cancellation.signal; }
  currentTime() {
    let time;
    try { time = this.now(); } catch { this.revoke(); throw new Error('Clock unavailable'); }
    if (!validTime(time) || time < this.lastTime || (this.grant && time >= this.grant.expiresAt)) {
      this.revoke(); throw new Error('Lease expired or clock invalid');
    }
    this.lastTime = time;
    return time;
  }
  // sessionDeadline comes from independently authenticated session context,
  // not a renderer field or a deadline chosen by the grant being admitted.
  bind(envelope, expected, sessionDeadline) {
    if (this.state !== 'inactive') throw new Error('Fresh endpoint required');
    let grant;
    try { grant = this.verify(envelope); } catch { this.revoke(); throw new Error("Grant verification failed"); }
    const time = this.currentTime();
    if (this.state !== 'inactive' || !grant || !expected ||
      !validTime(sessionDeadline) || sessionDeadline <= time ||
      BINDING.some(key => typeof grant[key] !== 'string' || grant[key].length > 320 ||
      !grant[key].length || grant[key] !== expected[key]) ||
      !validTime(grant.issuedAt) || !validTime(grant.expiresAt) ||
      grant.issuedAt > time || grant.expiresAt <= time || grant.expiresAt > sessionDeadline ||
      grant.expiresAt - grant.issuedAt > 900_000 ||
      !Number.isSafeInteger(grant.generation) || grant.generation < 1 ||
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
    this.currentTime();
    if (this.state !== 'consent-pending' || !Array.isArray(capabilities) ||
        !capabilities.length || capabilities.length > CAPABILITIES.length ||
        new Set(capabilities).size !== capabilities.length ||
        capabilities.some(cap => !this.grant.capabilities.includes(cap))) throw new Error('Consent refused');
    this.consent = new Set(capabilities);
    for (const capability of capabilities) this.capabilityCancellations.set(capability, new AbortController());
    this.state = 'session-bound';
  }
  assertAuthorized(capability) {
    this.currentTime();
    if (this.state !== 'session-bound' || !this.grant || !this.grant.capabilities.includes(capability) ||
        !this.consent.has(capability)) throw new Error('Operation refused');
    return this.grant;
  }
  signalFor(capability) {
    this.assertAuthorized(capability);
    return this.capabilityCancellations.get(capability).signal;
  }
  revokeCapabilities(capabilities) {
    if (this.state === 'terminated') return;
    this.currentTime();
    if (this.state !== 'session-bound' || !Array.isArray(capabilities) || capabilities.length > CAPABILITIES.length ||
        new Set(capabilities).size !== capabilities.length ||
        capabilities.some(cap => !this.grant.capabilities.includes(cap))) throw new Error('Revocation refused');
    // Invalidate the whole requested set before any cancellation listener runs.
    for (const capability of capabilities) {
      this.consent.delete(capability); this.sequences.delete(capability); this.rates.delete(capability);
    }
    for (const capability of capabilities) this.capabilityCancellations.get(capability)?.abort();
  }
  check(capability, message) {
    const grant = this.assertAuthorized(capability), time = this.lastTime;
    if (!message || message.sessionId !== grant.sessionId || message.grantId !== grant.grantId ||
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
    this.cancellation.abort();
    for (const cancellation of this.capabilityCancellations.values()) cancellation.abort();
    this.capabilityCancellations.clear();
    this.onRevoke();
  }
}
module.exports = { AssistLease, CAPABILITIES };
