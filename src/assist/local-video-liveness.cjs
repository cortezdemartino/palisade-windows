'use strict';
const { randomBytes } = require('node:crypto');
const { performance } = require('node:perf_hooks');
// Private component liveness only, never Assist authorization.
class Liveness {
  constructor(now = () => performance.now()) {
    this.now = now;
    this.sequence = 0;
    this.pending = new Map();
    this.verified = new Map();
  }
  expired(role) {
    const time = this.now(),
      pending = this.pending.get(role),
      verified = this.verified.get(role);
    return Boolean(
      (pending && (time < pending.at || time - pending.at >= 1500)) ||
        (verified !== undefined && (time < verified || time - verified >= 1500)),
    );
  }
  issue(role) {
    if (this.pending.has(role)) return null;
    if (this.sequence >= 0xffffffff) throw Error('Heartbeat exhausted');
    const value = {
      sequence: ++this.sequence,
      challenge: randomBytes(16).toString('hex'),
      at: this.now(),
    };
    this.pending.set(role, value);
    return value;
  }
  match(role, message, arrived) {
    const value = this.pending.get(role),
      now = this.now();
    return value &&
      message.sequence === value.sequence &&
      message.challenge === value.challenge &&
      arrived >= value.at &&
      arrived - value.at < 1500 &&
      now >= value.at &&
      now - value.at < 1500
      ? value
      : null;
  }
  finish(role, value, verified) {
    const now = this.now();
    if (this.pending.get(role) !== value || now < value.at || now - value.at >= 1500)
      throw Error('Stale heartbeat');
    this.pending.delete(role);
    if (verified) this.verified.set(role, value.at);
  }
}
module.exports = { Liveness };
